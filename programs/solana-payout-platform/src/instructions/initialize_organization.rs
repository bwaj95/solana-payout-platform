use anchor_lang::prelude::*;

use crate::{
    events::OrganizationInitialized, Member, MemberWallet, Organization,
    INITIAL_AUTHORIZATION_REVISION, INITIAL_OWNER_ROLES, MEMBER_SEED, MEMBER_WALLET_SEED,
    ORGANIZATION_SEED,
};

#[derive(Accounts)]
#[instruction(organization_id: u64, owner_member_id: u64)]
pub struct InitializeOrganization<'info> {
    #[account(mut)]
    pub creator: Signer<'info>,

    #[account(
        init,
        payer = creator,
        space = 8 + Organization::INIT_SPACE,
        seeds = [ORGANIZATION_SEED, creator.key().as_ref(), organization_id.to_le_bytes().as_ref()],
        bump
    )]
    pub organization: Account<'info, Organization>,

    #[account(
        init,
        payer = creator,
        space = 8 + Member::INIT_SPACE,
        seeds = [MEMBER_SEED, organization.key().as_ref(), owner_member_id.to_le_bytes().as_ref()],
        bump
    )]
    pub owner_member: Account<'info, Member>,

    #[account(
    init,
    payer = creator,
    space = 8 + MemberWallet::INIT_SPACE,
    seeds = [
        MEMBER_WALLET_SEED,
        organization.key().as_ref(),
        creator.key().as_ref(),
    ],
    bump
    )]
    pub owner_member_wallet: Account<'info, MemberWallet>,

    pub system_program: Program<'info, System>,
}

pub fn initialize_organization_handler(
    ctx: Context<InitializeOrganization>,
    organization_id: u64,
    owner_member_id: u64,
) -> Result<()> {
    let creator: Pubkey = ctx.accounts.creator.key();
    let organization: Pubkey = ctx.accounts.organization.key();
    let owner_member: Pubkey = ctx.accounts.owner_member.key();

    ctx.accounts.organization.set_inner(Organization {
        organization_id,
        creator,
        owner_member,
        paused: false,
        bump: ctx.bumps.organization,
    });

    ctx.accounts.owner_member.set_inner(Member {
        organization,
        member_id: owner_member_id,
        authorized_wallet: creator,
        authorization_revision: INITIAL_AUTHORIZATION_REVISION,
        roles: INITIAL_OWNER_ROLES,
        active: true,
        bump: ctx.bumps.owner_member,
    });

    ctx.accounts.owner_member_wallet.set_inner(MemberWallet {
        organization,
        member: owner_member,
        authorized_wallet: creator,
        bump: ctx.bumps.owner_member_wallet,
    });

    emit!(OrganizationInitialized {
        creator,
        organization,
        organization_id,
        owner_member,
        owner_member_id
    });

    Ok(())
}
