use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::{
    error::ErrorCode, Asset, Member, Organization, Recipient, VaultState,
    INITIAL_RECIPIENT_WALLET_REVISION, MEMBER_SEED, ORGANIZATION_SEED, RECIPIENT_SEED, ROLE_ADMIN,
    ROLE_PREPARER, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(recipient_id: u64, destination_wallet: Pubkey)]
pub struct RegisterRecipient<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

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

    #[account(
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

    #[account(
        init,
        payer = authority,
        space = 8 + Recipient::INIT_SPACE,
        seeds = [
            RECIPIENT_SEED,
            organization.key().as_ref(),
            recipient_id.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub recipient: Account<'info, Recipient>,

    // Account<Mint> ensures this is a legacy SPL Token mint.
    pub mint: Account<'info, Mint>,

    #[account(
        constraint = destination_token_account.mint == mint.key()
            @ ErrorCode::RecipientTokenMintMismatch,
        constraint = destination_token_account.owner == destination_wallet
            @ ErrorCode::RecipientTokenOwnerMismatch,
    )]
    pub destination_token_account: Account<'info, TokenAccount>,

    // Required because Anchor creates the Recipient PDA.
    pub system_program: Program<'info, System>,
}

pub fn register_recipient_handler(
    ctx: Context<RegisterRecipient>,
    recipient_id: u64,
    destination_wallet: Pubkey,
) -> Result<()> {
    require!(recipient_id > 0, ErrorCode::InvalidRecipientId);

    require_keys_neq!(
        destination_wallet,
        Pubkey::default(),
        ErrorCode::InvalidRecipientDestination
    );

    // The client cannot select an arbitrary mint. The mint must be the
    // SPL asset configured in this organization's supplied vault.
    let vault_mint = match &ctx.accounts.vault_state.asset {
        Asset::Spl { mint } => *mint,
        Asset::NativeSol => {
            return err!(ErrorCode::UnsupportedVaultAsset);
        }
    };

    require_keys_eq!(
        vault_mint,
        ctx.accounts.mint.key(),
        ErrorCode::VaultMintMismatch
    );

    // Checking only the token account's mint and authority is insufficient:
    // a wallet may own multiple token accounts for the same mint. Require the
    // standard ATA derived from wallet + token program + mint.
    let mint_key = ctx.accounts.mint.key();

    let (expected_destination_token_account, _) = Pubkey::find_program_address(
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

    ctx.accounts.recipient.set_inner(Recipient {
        organization: ctx.accounts.organization.key(),
        recipient_id,
        current_destination: destination_wallet,
        wallet_revision: INITIAL_RECIPIENT_WALLET_REVISION,
        active: true,
        bump: ctx.bumps.recipient,
    });

    Ok(())
}
