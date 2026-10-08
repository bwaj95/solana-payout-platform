use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    error::ErrorCode,
    events::SplPaymentExecuted,
    instructions::approval_validation::{get_vault_mint_from_vault_state, validate_payment_terms},
    transfer_tokens_checked_with_signer, Member, Organization, Payment, PaymentState, Recipient,
    ReservationState, VaultState, MEMBER_SEED, ORGANIZATION_SEED, PAYMENT_SEED, RECIPIENT_SEED,
    ROLE_EXECUTOR, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(payment_id: u64)]
pub struct ExecuteSplPayment<'info> {
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
    pub organization: Box<Account<'info, Organization>>,

    #[account(
        seeds = [
            MEMBER_SEED,
            organization.key().as_ref(),
            executor_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = executor_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = executor_member.active
            @ ErrorCode::InactiveMember,
        constraint = executor_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,

        constraint = (executor_member.roles & ROLE_EXECUTOR) != 0
            @ ErrorCode::MissingPaymentExecutorRole,
    )]
    pub executor_member: Box<Account<'info, Member>>,

    pub mint: Box<Account<'info, Mint>>,

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
    pub vault_state: Box<Account<'info, VaultState>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = vault_state,
    )]
    pub vault_ata: Box<Account<'info, TokenAccount>>,

    #[account(
        seeds = [
            RECIPIENT_SEED,
            organization.key().as_ref(),
            recipient.recipient_id.to_le_bytes().as_ref(),
        ],
        bump = recipient.bump,
        has_one = organization
            @ ErrorCode::RecipientOrganizationMismatch,
        constraint = recipient.active
            @ ErrorCode::InactiveRecipient,
    )]
    pub recipient: Box<Account<'info, Recipient>>,

    #[account(
        mut,
        associated_token::mint = mint,
        associated_token::authority = recipient.current_destination
    )]
    pub destination_ata: Box<Account<'info, TokenAccount>>,

    // Passing the policy account here only means that the client has passed the policy referenced by the payment.
    // Once payment is approved and reservation is held, we can proceed with the payment.
    // checking approval_policy_version.enabled here means a payment can be cancelled from executing just by pausing the policy.
    // we should have explicit instructions for payment cancellation.
    // An admin should stop an approved payment through organization pause or an explicit cancellation instruction,
    // not indirectly by disabling its old policy version.
    // pub approval_policy_version: Box<Account<'info, ApprovalPolicyVersion>>,

    // payment doesn't reference anything from approval_policy_version
    #[account(
        mut,
        seeds = [
            PAYMENT_SEED,
            organization.key().as_ref(),
            payment.payment_id.to_le_bytes().as_ref(),
        ],
        bump = payment.bump,
        has_one = organization
            @ ErrorCode::PaymentTermsMismatch,

        constraint = payment.payment_id == payment_id
            @ ErrorCode::InvalidPaymentId,

        constraint = payment.recipient == recipient.key()
            @ ErrorCode::PaymentTermsMismatch,
        constraint = payment.destination == recipient.current_destination
            @ ErrorCode::PaymentTermsMismatch,
        constraint = payment.recipient_wallet_revision == recipient.wallet_revision
            @ ErrorCode::PaymentTermsMismatch,

        // constraint = payment.policy_version == approval_policy_version.key()
        //     @ ErrorCode::PaymentTermsMismatch,
        constraint = payment.vault == vault_state.key()
            @ ErrorCode::PaymentTermsMismatch,

        constraint = payment.payment_state == PaymentState::Approved
         @ ErrorCode::InvalidPaymentApprovalState,

        constraint = payment.reservation_state == ReservationState::Held
            @ ErrorCode::InvalidPaymentReservationExecutionState,
    )]
    pub payment: Box<Account<'info, Payment>>,

    pub token_program: Program<'info, Token>,
}

pub fn execute_spl_payment_handler(
    ctx: Context<ExecuteSplPayment>,
    _payment_id: u64,
) -> Result<()> {
    let executed_at = Clock::get()?.unix_timestamp;

    require!(
        executed_at >= ctx.accounts.payment.execute_after,
        ErrorCode::ExecutionTimeNotReached
    );

    let vault_mint = get_vault_mint_from_vault_state(&ctx.accounts.vault_state)?;

    require_keys_eq!(
        vault_mint,
        ctx.accounts.mint.key(),
        ErrorCode::VaultMintMismatch
    );

    validate_payment_terms(
        ctx.accounts.organization.key(),
        &ctx.accounts.payment,
        &vault_mint,
    )?;

    let amount = ctx.accounts.payment.amount;

    // This payment must have an amount included in the aggregate reservation.
    let remaining_reserved_total = ctx
        .accounts
        .vault_state
        .reserved_total
        .checked_sub(amount)
        .ok_or(ErrorCode::VaultReservationInsufficient)?;

    require!(
        ctx.accounts.vault_ata.amount >= ctx.accounts.vault_state.reserved_total,
        ErrorCode::InsufficientVaultBalance
    );

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
        &ctx.accounts.destination_ata.to_account_info(),
        &ctx.accounts.mint.to_account_info(),
        &ctx.accounts.token_program.to_account_info(),
        amount,
        ctx.accounts.mint.decimals,
        signer_seeds,
    )?;

    ctx.accounts.vault_state.reserved_total = remaining_reserved_total;

    ctx.accounts.payment.payment_state = PaymentState::Paid;
    ctx.accounts.payment.reservation_state = ReservationState::Consumed;

    emit!(SplPaymentExecuted {
        organization: ctx.accounts.organization.key(),
        payment: ctx.accounts.payment.key(),
        payment_id: ctx.accounts.payment.payment_id,
        payment_revision: ctx.accounts.payment.payment_revision,
        vault: ctx.accounts.vault_state.key(),
        recipient: ctx.accounts.recipient.key(),
        destination_wallet: ctx.accounts.recipient.current_destination,
        destination_token_account: ctx.accounts.destination_ata.key(),
        executor_member: ctx.accounts.executor_member.key(),
        executor_wallet: ctx.accounts.authority.key(),
        mint: ctx.accounts.mint.key(),
        amount,
        remaining_reserved_total,
        executed_at,
    });

    Ok(())
}
