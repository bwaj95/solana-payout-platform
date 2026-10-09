use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::{
    error::ErrorCode, events::RecipientWalletRotated, Asset, Member, Organization, Recipient,
    VaultState, MEMBER_SEED, ORGANIZATION_SEED, RECIPIENT_SEED, ROLE_ADMIN, ROLE_PREPARER,
    VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(
    recipient_id: u64,
    new_destination_wallet: Pubkey
)]
pub struct RotateRecipientWallet<'info> {
    pub authority: Signer<'info>,

    /*
     * Rotation is allowed while the organization is paused.
     *
     * Pausing prevents financial execution, but administrators must still
     * be able to perform recovery operations such as rotating a compromised
     * recipient wallet.
     */
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
            registrar_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = registrar_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = registrar_member.active
            @ ErrorCode::InactiveMember,
        constraint = registrar_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (
            registrar_member.roles & (ROLE_ADMIN | ROLE_PREPARER)
        ) != 0
            @ ErrorCode::MissingRecipientRegistrarRole,
    )]
    pub registrar_member: Account<'info, Member>,

    /*
     * The vault identifies the mint for which the new destination ATA must
     * already exist.
     *
     * We do not require vault_state.active because wallet recovery should
     * remain possible while a vault is administratively disabled.
     */
    #[account(
        seeds = [
            VAULT_SEED,
            organization.key().as_ref(),
            vault_state.vault_id.to_le_bytes().as_ref(),
        ],
        bump = vault_state.bump,
        has_one = organization
            @ ErrorCode::VaultOrganizationMismatch,
    )]
    pub vault_state: Account<'info, VaultState>,

    #[account(
        mut,
        seeds = [
            RECIPIENT_SEED,
            organization.key().as_ref(),
            recipient_id.to_le_bytes().as_ref(),
        ],
        bump = recipient.bump,
        has_one = organization
            @ ErrorCode::RecipientOrganizationMismatch,
        constraint = recipient.recipient_id == recipient_id
            @ ErrorCode::InvalidRecipientId,
    )]
    pub recipient: Account<'info, Recipient>,

    pub mint: Account<'info, Mint>,

    /*
     * The new wallet must already own a token account for this mint.
     * The handler additionally proves that this is its canonical ATA.
     */
    #[account(
        constraint = new_destination_token_account.mint == mint.key()
            @ ErrorCode::RecipientTokenMintMismatch,
        constraint = (
            new_destination_token_account.owner
                == new_destination_wallet
        )
            @ ErrorCode::RecipientTokenOwnerMismatch,
    )]
    pub new_destination_token_account: Account<'info, TokenAccount>,
}

pub fn rotate_recipient_wallet_handler(
    ctx: Context<RotateRecipientWallet>,
    recipient_id: u64,
    new_destination_wallet: Pubkey,
) -> Result<()> {
    require!(recipient_id > 0, ErrorCode::InvalidRecipientId);

    require_keys_neq!(
        new_destination_wallet,
        Pubkey::default(),
        ErrorCode::InvalidRecipientDestination
    );

    let previous_destination = ctx.accounts.recipient.current_destination;

    require_keys_neq!(
        new_destination_wallet,
        previous_destination,
        ErrorCode::RecipientDestinationUnchanged
    );

    /*
     * The client cannot provide an arbitrary mint. It must match the SPL
     * asset configured in the supplied organization vault.
     */
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
     * A wallet may own multiple token accounts for the same mint. Require
     * the standard associated token account rather than accepting any token
     * account owned by the new wallet.
     */
    let mint_key = ctx.accounts.mint.key();

    let (expected_destination_token_account, _destination_ata_bump) = Pubkey::find_program_address(
        &[
            new_destination_wallet.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint_key.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    );

    require_keys_eq!(
        ctx.accounts.new_destination_token_account.key(),
        expected_destination_token_account,
        ErrorCode::InvalidRecipientTokenAccount
    );

    /*
     * Calculate the new revision before changing account state so every
     * fallible validation is complete first.
     */
    let previous_wallet_revision = ctx.accounts.recipient.wallet_revision;

    let new_wallet_revision = previous_wallet_revision
        .checked_add(1)
        .ok_or(ErrorCode::ArithmeticOverflow)?;

    let rotated_at = Clock::get()?.unix_timestamp;

    ctx.accounts.recipient.current_destination = new_destination_wallet;

    ctx.accounts.recipient.wallet_revision = new_wallet_revision;

    emit!(RecipientWalletRotated {
        organization: ctx.accounts.organization.key(),
        recipient: ctx.accounts.recipient.key(),
        recipient_id: ctx.accounts.recipient.recipient_id,

        previous_destination,
        new_destination: new_destination_wallet,

        previous_wallet_revision,
        new_wallet_revision,

        rotated_by_member: ctx.accounts.registrar_member.key(),
        rotated_by_wallet: ctx.accounts.authority.key(),

        rotated_at,
    });

    Ok(())
}
