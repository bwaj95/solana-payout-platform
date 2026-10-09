use anchor_lang::prelude::*;

use crate::{
    error::ErrorCode, events::OrganizationPauseStateChanged, Member, Organization, MEMBER_SEED,
    ORGANIZATION_SEED, ROLE_ADMIN,
};

#[derive(Accounts)]
pub struct SetOrganizationPaused<'info> {
    pub authority: Signer<'info>,

    /*
     * Do not add `!organization.paused` here.
     *
     * This same instruction must work while paused so an administrator can
     * unpause the organization.
     */
    #[account(
        mut,
        seeds = [
            ORGANIZATION_SEED,
            organization.creator.as_ref(),
            organization.organization_id.to_le_bytes().as_ref(),
        ],
        bump = organization.bump,
    )]
    pub organization: Account<'info, Organization>,

    #[account(
        seeds = [
            MEMBER_SEED,
            organization.key().as_ref(),
            admin_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = admin_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = admin_member.active
            @ ErrorCode::InactiveMember,
        constraint = admin_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (admin_member.roles & ROLE_ADMIN) != 0
            @ ErrorCode::MissingAdminRole,
    )]
    pub admin_member: Account<'info, Member>,
}

pub fn set_organization_paused_handler(
    ctx: Context<SetOrganizationPaused>,
    paused: bool,
) -> Result<()> {
    let previous_paused = ctx.accounts.organization.paused;

    require!(
        previous_paused != paused,
        ErrorCode::OrganizationPauseStateUnchanged
    );

    let changed_at = Clock::get()?.unix_timestamp;

    ctx.accounts.organization.paused = paused;

    emit!(OrganizationPauseStateChanged {
        organization: ctx.accounts.organization.key(),

        previous_paused,
        new_paused: paused,

        changed_by_member: ctx.accounts.admin_member.key(),
        changed_by_wallet: ctx.accounts.authority.key(),

        changed_at,
    });

    Ok(())
}
