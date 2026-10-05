use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode, events::MemberCreated, Member, MemberWallet, Organization,
    ASSIGNABLE_MEMBER_ROLES, INITIAL_AUTHORIZATION_REVISION, MEMBER_SEED, MEMBER_WALLET_SEED,
    ORGANIZATION_SEED, ROLE_ADMIN,
};

#[derive(Accounts)]
#[instruction(member_id: u64, authorized_wallet: Pubkey)]
pub struct CreateMember<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        seeds = [ORGANIZATION_SEED, organization.creator.as_ref(), organization.organization_id.to_le_bytes().as_ref()],
        bump = organization.bump,
    )]
    pub organization: Account<'info, Organization>,

    // The wallet stored inside the admin_member pda is the admin/org-owner and should sign.
    // The admin_member pda cannot sign.
    // The authority wallet signs, while admin_member proves that the signer
    // is an active administrator of this Organization.
    // The Member PDA itself cannot sign.
    #[account(
        seeds = [MEMBER_SEED, organization.key().as_ref(), admin_member.member_id.to_le_bytes().as_ref()],
        bump = admin_member.bump,
        has_one = organization @ ErrorCode::MemberOrganizationMismatch,
        constraint = admin_member.active @ ErrorCode::InactiveMember,
        constraint = admin_member.authorized_wallet == authority.key() @ ErrorCode::UnauthorizedWallet,
        constraint = (admin_member.roles & ROLE_ADMIN) != 0 @ ErrorCode::MissingAdminRole

    )]
    pub admin_member: Account<'info, Member>,

    #[account(
        init,
        payer = authority,
        space = 8 + Member::INIT_SPACE,
        seeds = [MEMBER_SEED, organization.key().as_ref(), member_id.to_le_bytes().as_ref()],
        bump
    )]
    pub member: Account<'info, Member>,

    #[account(
        init,
        payer = authority,
        space = 8 + MemberWallet::INIT_SPACE,
        seeds = [MEMBER_WALLET_SEED, organization.key().as_ref(), authorized_wallet.as_ref()],
        bump
    )]
    pub member_wallet: Account<'info, MemberWallet>,

    pub system_program: Program<'info, System>,
}

pub fn create_member_handler(
    ctx: Context<CreateMember>,
    member_id: u64,
    authorized_wallet: Pubkey,
    roles: u16,
) -> Result<()> {
    require_keys_neq!(
        authorized_wallet,
        Pubkey::default(),
        ErrorCode::InvalidAuthorizedWallet
    );

    require!(
        roles != 0 && (roles & !ASSIGNABLE_MEMBER_ROLES) == 0,
        ErrorCode::InvalidMemberRoles
    );

    // Member and MemberWallet are created in the same Solana transaction.
    // If either PDA initialization, validation, or state write fails, the
    // runtime rolls back both accounts. This prevents an unindexed Member
    // or a wallet index pointing to a nonexistent Member.

    ctx.accounts.member.set_inner(Member {
        organization: ctx.accounts.organization.key(),
        member_id,
        authorized_wallet,
        authorization_revision: INITIAL_AUTHORIZATION_REVISION,
        roles,
        active: true,
        bump: ctx.bumps.member,
    });

    ctx.accounts.member_wallet.set_inner(MemberWallet {
        organization: ctx.accounts.organization.key(),
        member: ctx.accounts.member.key(),
        authorized_wallet,
        bump: ctx.bumps.member_wallet,
    });

    emit!(MemberCreated {
        organization: ctx.accounts.organization.key(),
        member: ctx.accounts.member.key(),
        member_wallet: ctx.accounts.member_wallet.key(),
        member_id,
        authorized_wallet,
        roles,
        created_by_member: ctx.accounts.admin_member.key(),
        created_by_wallet: ctx.accounts.authority.key(),
    });

    Ok(())
}
