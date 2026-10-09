use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    error::ErrorCode, events::VaultFundsWithdrawn, transfer_tokens_checked_with_signer, Asset,
    Member, Organization, VaultState, MEMBER_SEED, ORGANIZATION_SEED, ROLE_ADMIN, ROLE_TREASURY,
    VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(amount: u64, destination_wallet: Pubkey)]
pub struct WithdrawSplVaultFunds<'info> {
    pub authority: Signer<'info>,

    /*
     * Withdrawal is a financial outflow, so it must be blocked while the
     * organization is paused.
     */
    #[account(
        seeds = [
            ORGANIZATION_SEED,
            organization.creator.as_ref(),
            organization.organization_id.to_le_bytes().as_ref(),
        ],
        bump = organization.bump,
        constraint = !organization.paused
            @ ErrorCode::OrganizationPaused,
    )]
    pub organization: Account<'info, Organization>,

    #[account(
        seeds = [
            MEMBER_SEED,
            organization.key().as_ref(),
            treasury_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = treasury_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = treasury_member.active
            @ ErrorCode::InactiveMember,
        constraint = treasury_member.authorized_wallet
            == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (
            treasury_member.roles
                & (ROLE_ADMIN | ROLE_TREASURY)
        ) != 0
            @ ErrorCode::MissingVaultWithdrawalRole,
    )]
    pub treasury_member: Account<'info, Member>,

    #[account(
        mut,
        seeds = [
            VAULT_SEED,
            organization.key().as_ref(),
            vault_state.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault_state.bump,
        has_one = organization
            @ ErrorCode::VaultOrganizationMismatch,
        constraint = vault_state.active
            @ ErrorCode::InactiveVault,
    )]
    pub vault_state: Account<'info, VaultState>,

    pub mint: Account<'info, Mint>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault_state,
    )]
    pub vault_ata: Box<Account<'info, TokenAccount>>,

    /*
     * The destination wallet does not need to sign. An authorized treasury
     * member selects the destination, while these constraints ensure that
     * the supplied token account belongs to that wallet and uses this mint.
     */
    #[account(
        mut,
        constraint = destination_token_account.mint == mint.key()
            @ ErrorCode::RecipientTokenMintMismatch,
        constraint = destination_token_account.owner
            == destination_wallet
            @ ErrorCode::RecipientTokenOwnerMismatch,
    )]
    pub destination_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn withdraw_spl_vault_funds_handler(
    ctx: Context<WithdrawSplVaultFunds>,
    amount: u64,
    destination_wallet: Pubkey,
) -> Result<()> {
    require!(amount > 0, ErrorCode::InvalidVaultWithdrawalAmount);

    require_keys_neq!(
        destination_wallet,
        Pubkey::default(),
        ErrorCode::InvalidVaultWithdrawalDestination
    );

    /*
     * Withdrawing back into the vault's own ATA would be a token-program
     * no-op but could produce a misleading withdrawal event.
     */
    require_keys_neq!(
        destination_wallet,
        ctx.accounts.vault_state.key(),
        ErrorCode::InvalidVaultWithdrawalDestination
    );

    require_keys_neq!(
        ctx.accounts.destination_token_account.key(),
        ctx.accounts.vault_ata.key(),
        ErrorCode::InvalidVaultWithdrawalDestination
    );

    let vault_mint = match ctx.accounts.vault_state.asset {
        Asset::Spl { mint } => mint,
        Asset::NativeSol => {
            return err!(ErrorCode::UnsupportedVaultAsset);
        }
    };

    require_keys_eq!(
        vault_mint,
        ctx.accounts.mint.key(),
        ErrorCode::VaultMintMismatch
    );

    /*
     * Require the standard ATA for destination_wallet + mint. Merely checking
     * the token account's internal owner and mint would still allow a custom
     * non-associated token account.
     */
    let mint_key = ctx.accounts.mint.key();

    let (expected_destination_token_account, _destination_ata_bump) = Pubkey::find_program_address(
        &[
            destination_wallet.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint_key.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    );

    require_keys_eq!(
        ctx.accounts.destination_token_account.key(),
        expected_destination_token_account,
        ErrorCode::InvalidRecipientTokenAccount
    );

    let vault_balance_before = ctx.accounts.vault_ata.amount;

    let reserved_total = ctx.accounts.vault_state.reserved_total;

    /*
     * This subtraction verifies the vault accounting invariant first.
     *
     * If reserved_total is greater than the real token balance, the vault
     * state is inconsistent and no withdrawal should be allowed.
     */
    let available_unreserved_balance = vault_balance_before
        .checked_sub(reserved_total)
        .ok_or(ErrorCode::VaultReservationInvariantViolation)?;

    require!(
        amount <= available_unreserved_balance,
        ErrorCode::VaultWithdrawalExceedsAvailableBalance
    );

    /*
     * Calculate the event's post-transfer balance before the CPI. All
     * fallible arithmetic is therefore completed before token movement.
     */
    let vault_balance_after = vault_balance_before
        .checked_sub(amount)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let organization_key = ctx.accounts.organization.key();

    let vault_id_bytes = ctx.accounts.vault_state.vault_id.to_le_bytes();

    let vault_bump_seed = [ctx.accounts.vault_state.bump];

    let vault_signer_seeds: &[&[u8]] = &[
        VAULT_SEED,
        organization_key.as_ref(),
        vault_id_bytes.as_ref(),
        vault_bump_seed.as_ref(),
    ];

    let signer_seeds: &[&[&[u8]]] = &[vault_signer_seeds];

    transfer_tokens_checked_with_signer(
        &ctx.accounts.vault_state.to_account_info(),
        &ctx.accounts.vault_ata.to_account_info(),
        &ctx.accounts.destination_token_account.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        amount,
        ctx.accounts.mint.decimals,
        signer_seeds,
    )?;

    let withdrawn_at = Clock::get()?.unix_timestamp;

    /*
     * reserved_total is intentionally unchanged. This instruction transfers
     * only funds that are outside the reserved portion.
     */
    emit!(VaultFundsWithdrawn {
        organization: ctx.accounts.organization.key(),
        vault: ctx.accounts.vault_state.key(),
        vault_id: ctx.accounts.vault_state.vault_id,

        mint: ctx.accounts.mint.key(),

        destination_wallet,
        destination_token_account: ctx.accounts.destination_token_account.key(),

        withdrawn_by_member: ctx.accounts.treasury_member.key(),
        withdrawn_by_wallet: ctx.accounts.authority.key(),

        amount,

        vault_balance_before,
        vault_balance_after,
        reserved_total,

        withdrawn_at,
    });

    Ok(())
}
