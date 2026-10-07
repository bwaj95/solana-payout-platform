use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::{
    error::ErrorCode,
    events::PaymentApprovalThresholdReached,
    instructions::approval_validation::{
        process_threshold_and_reservation, validate_and_count_approval_witnesses,
        validate_payment_terms, validate_policy,
    },
    ApprovalPolicyVersion, Asset, Member, Organization, Payment, PaymentState, Recipient,
    ReservationState, VaultState, MEMBER_SEED, ORGANIZATION_SEED, PAYMENT_SEED, POLICY_SEED,
    RECIPIENT_SEED, VAULT_SEED,
};
use crate::{ROLE_ADMIN, ROLE_EXECUTOR};

#[derive(Accounts)]
#[instruction(payment_id: u64)]
pub struct FinalizePaymentApproval<'info> {
    /*
     * Finalization creates no account, so authority does not pay rent here.
     * It still signs to prove control of the registered finalizer member.
     */
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

    /*
     * Finalization is an objective state transition, so any active registered
     * organization member may trigger it. They do not become an approver merely
     * by calling this instruction.
     */
    #[account(
    seeds = [
        MEMBER_SEED,
        organization.key().as_ref(),
        finalizer_member.member_id.to_le_bytes().as_ref(),
    ],
    bump = finalizer_member.bump,
    has_one = organization
        @ ErrorCode::MemberOrganizationMismatch,
    constraint = finalizer_member.active
        @ ErrorCode::InactiveMember,
    constraint = finalizer_member.authorized_wallet == authority.key()
        @ ErrorCode::UnauthorizedWallet,
    constraint = (
        finalizer_member.roles & (ROLE_ADMIN | ROLE_EXECUTOR)
    ) != 0
        @ ErrorCode::MissingPaymentFinalizerRole,
)]
    pub finalizer_member: Account<'info, Member>,

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

    pub mint: Box<Account<'info, Mint>>,

    #[account(
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
        seeds = [
            POLICY_SEED,
            organization.key().as_ref(),
            approval_policy_version.policy_id.to_le_bytes().as_ref(),
            approval_policy_version.version.to_le_bytes().as_ref(),
        ],
        bump = approval_policy_version.bump,
        has_one = organization
            @ ErrorCode::PolicyOrganizationMismatch,
        constraint = approval_policy_version.enabled
            @ ErrorCode::InactivePolicy,
    )]
    pub approval_policy_version: Box<Account<'info, ApprovalPolicyVersion>>,

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

        constraint = payment.policy_version == approval_policy_version.key()
            @ ErrorCode::PaymentTermsMismatch,
        constraint = payment.vault == vault_state.key()
            @ ErrorCode::PaymentTermsMismatch,

        constraint = (
            payment.payment_state == PaymentState::PendingApproval
                || payment.payment_state == PaymentState::AwaitingFunds
        ) @ ErrorCode::PaymentNotAcceptingApprovals,

        constraint = payment.reservation_state == ReservationState::None
            @ ErrorCode::InvalidPaymentReservationState,
    )]
    pub payment: Box<Account<'info, Payment>>,
}

pub fn finalize_payment_approval_handler(
    ctx: Context<FinalizePaymentApproval>,
    _payment_id: u64,
) -> Result<()> {
    validate_policy(&ctx.accounts.approval_policy_version)?;

    let vault_mint = match ctx.accounts.vault_state.asset {
        Asset::Spl { mint } => mint,
        Asset::NativeSol => {
            return err!(ErrorCode::UnsupportedVaultAsset);
        }
    };

    validate_payment_terms(
        ctx.accounts.organization.key(),
        &ctx.accounts.payment,
        &vault_mint,
    )?;

    require_keys_eq!(
        vault_mint,
        ctx.accounts.mint.key(),
        ErrorCode::VaultMintMismatch
    );

    /*
     * Unlike approve_payment, there is no newly created Approval to count.
     * Every approval must therefore be provided through remaining_accounts.
     */
    let valid_approval_count = validate_and_count_approval_witnesses(
        ctx.remaining_accounts,
        ctx.accounts.organization.key(),
        ctx.accounts.payment.key(),
        &ctx.accounts.payment,
        ctx.accounts.approval_policy_version.key(),
        &ctx.accounts.approval_policy_version,
        None,
    )?;

    require!(
        valid_approval_count >= ctx.accounts.approval_policy_version.threshold,
        ErrorCode::ApprovalThresholdNotMet
    );

    let outcome = process_threshold_and_reservation(
        &mut ctx.accounts.payment,
        &mut ctx.accounts.vault_state,
        ctx.accounts.vault_ata.amount,
        valid_approval_count,
        ctx.accounts.approval_policy_version.threshold,
    )?;

    /*
     * The explicit threshold require above means this should always be true.
     * Keeping this assertion documents the expected helper contract.
     */
    require!(
        outcome.threshold_reached,
        ErrorCode::ApprovalThresholdNotMet
    );

    let processed_at = Clock::get()?.unix_timestamp;

    emit!(PaymentApprovalThresholdReached {
        organization: ctx.accounts.organization.key(),
        payment: ctx.accounts.payment.key(),
        policy_version: ctx.accounts.approval_policy_version.key(),
        vault: ctx.accounts.vault_state.key(),
        triggered_by_member: ctx.accounts.finalizer_member.key(),
        valid_approval_count,
        threshold: ctx.accounts.approval_policy_version.threshold,
        amount: ctx.accounts.payment.amount,
        funds_reserved: outcome.funds_reserved,
        vault_reserved_total: ctx.accounts.vault_state.reserved_total,
        payment_state: ctx.accounts.payment.payment_state,
        processed_at,
    });

    Ok(())
}
