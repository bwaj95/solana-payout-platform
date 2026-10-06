use anchor_lang::prelude::*;
use anchor_spl::{
    associated_token::AssociatedToken,
    token::{Mint, Token, TokenAccount},
};

use crate::{
    error::ErrorCode, Asset, Member, Organization, VaultState, MEMBER_SEED, ORGANIZATION_SEED,
    ROLE_ADMIN, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(vault_id: u64)]
pub struct InitializeVault<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
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
        has_one = organization @ ErrorCode::MemberOrganizationMismatch,
        constraint = admin_member.active @ ErrorCode::InactiveMember,
        constraint = admin_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (admin_member.roles & ROLE_ADMIN) != 0
            @ ErrorCode::MissingAdminRole,
    )]
    pub admin_member: Account<'info, Member>,

    #[account(
        init,
        payer = authority,
        space = 8 + VaultState::INIT_SPACE,
        seeds = [
            VAULT_SEED,
            organization.key().as_ref(),
            vault_id.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub vault_state: Account<'info, VaultState>,

    // Account<Mint> also ensures this belongs to the legacy SPL Token Program.
    pub mint: Account<'info, Mint>,

    #[account(
        init,
        payer = authority,
        associated_token::mint = mint,
        associated_token::authority = vault_state,
        associated_token::token_program = token_program,
    )]
    pub vault_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_vault_handler(ctx: Context<InitializeVault>, vault_id: u64) -> Result<()> {
    require!(vault_id > 0, ErrorCode::InvalidVaultId);

    ctx.accounts.vault_state.set_inner(VaultState {
        organization: ctx.accounts.organization.key(),
        vault_id,
        asset: Asset::Spl {
            mint: ctx.accounts.mint.key(),
        },
        reserved_total: 0,
        active: true,
        bump: ctx.bumps.vault_state,
    });

    Ok(())
}
