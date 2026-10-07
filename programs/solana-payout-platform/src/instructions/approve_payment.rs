use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::{
    error::ErrorCode,
    events::{PaymentApprovalRecorded, PaymentApprovalThresholdReached},
    instructions::approval_validation::{
        process_threshold_and_reservation, validate_and_count_approval_witnesses,
        validate_payment_terms, validate_policy,
    },
    Approval, ApprovalPolicyVersion, Asset, Member, Organization, Payment, PaymentState, Recipient,
    ReservationState, VaultState, APPROVAL_SEED, MEMBER_SEED, ORGANIZATION_SEED, PAYMENT_SEED,
    POLICY_SEED, RECIPIENT_SEED, ROLE_APPROVER, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(payment_id: u64)]
pub struct ApprovePayment<'info> {
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
    pub organization: Box<Account<'info, Organization>>,

    #[account(
        seeds = [
            MEMBER_SEED,
            organization.key().as_ref(),
            approver_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = approver_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = approver_member.active
            @ ErrorCode::InactiveMember,
        constraint = approver_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,

        // ADMIN by itself is deliberately insufficient. An admin who should
        // approve must also hold ROLE_APPROVER and be listed in this policy.
        constraint = (approver_member.roles & ROLE_APPROVER) != 0
            @ ErrorCode::MissingPaymentApproverRole,
    )]
    pub approver_member: Box<Account<'info, Member>>,

    /*
     * vault_state and payment are writable because this instruction may reach
     * the threshold and reserve funds atomically.
     */
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

    /*
     * Anchor validates that this is the canonical ATA for:
     *
     * owner/authority = VaultState PDA
     * mint            = supplied Mint account
     *
     * The handler additionally checks that this mint matches
     * VaultState::asset.
     */
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

    /*
     * Payment must come before Approval because Approval's PDA seeds depend
     * upon fields deserialized from Payment.
     */
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

    /*
     * Organization namespacing prevents two organizations with identical
     * payment/member IDs from deriving the same Approval PDA.
     *
     * Both revisions are included so changed payment terms or wallet rotation
     * derive a new Approval PDA.
     */
    #[account(
        init,
        payer = authority,
        space = 8 + Approval::INIT_SPACE,
        seeds = [
            APPROVAL_SEED,
            organization.key().as_ref(),
            payment.payment_id.to_le_bytes().as_ref(),
            payment.payment_revision.to_le_bytes().as_ref(),
            approver_member.member_id.to_le_bytes().as_ref(),
            approver_member.authorization_revision.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub approval: Box<Account<'info, Approval>>,

    pub system_program: Program<'info, System>,
}

pub fn approve_payment_handler(ctx: Context<ApprovePayment>, _payment_id: u64) -> Result<()> {
    validate_policy(&ctx.accounts.approval_policy_version)?;

    /*
     * A role and policy membership are separate permissions:
     *
     * ROLE_APPROVER means the member is generally allowed to approve.
     * eligible_members means this exact policy selected the member.
     */
    require!(
        ctx.accounts
            .approval_policy_version
            .eligible_members
            .contains(&ctx.accounts.approver_member.key()),
        ErrorCode::ApproverNotEligibleForPolicy
    );

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

    /*
     * associated_token::mint validates the ATA against the supplied Mint.
     * This check connects that supplied Mint back to VaultState::asset.
     */
    require_keys_eq!(
        vault_mint,
        ctx.accounts.mint.key(),
        ErrorCode::VaultMintMismatch
    );

    /*
     * Validate previous approvals supplied by the client.
     *
     * The current approver is inserted into seen_members before validation so
     * an old Approval belonging to this same Member cannot be counted again.
     */
    let previous_valid_approval_count = validate_and_count_approval_witnesses(
        ctx.remaining_accounts,
        ctx.accounts.organization.key(),
        ctx.accounts.payment.key(),
        &ctx.accounts.payment,
        ctx.accounts.approval_policy_version.key(),
        &ctx.accounts.approval_policy_version,
        Some(ctx.accounts.approver_member.key()),
    )?;

    // The current approver has passed all typed-account constraints and policy
    // membership checks, so their newly created Approval contributes one.
    let valid_approval_count = previous_valid_approval_count
        .checked_add(1)
        .ok_or(error!(ErrorCode::ArithmeticOverflow))?;

    let approved_at = Clock::get()?.unix_timestamp;

    ctx.accounts.approval.set_inner(Approval {
        organization: ctx.accounts.organization.key(),
        payment: ctx.accounts.payment.key(),
        payment_revision: ctx.accounts.payment.payment_revision,
        policy: ctx.accounts.approval_policy_version.key(),
        member: ctx.accounts.approver_member.key(),
        member_authorization_revision: ctx.accounts.approver_member.authorization_revision,
        authorization_wallet: ctx.accounts.approver_member.authorized_wallet,
        approved_at,
        terms_hash: ctx.accounts.payment.terms_hash,
        bump: ctx.bumps.approval,
    });

    /*
     * If the threshold is reached, reserve immediately.
     *
     * If liquidity is insufficient, this does not return an error. It records
     * the approval and moves the Payment to AwaitingFunds.
     */
    let reservation_outcome = process_threshold_and_reservation(
        &mut ctx.accounts.payment,
        &mut ctx.accounts.vault_state,
        ctx.accounts.vault_ata.amount,
        valid_approval_count,
        ctx.accounts.approval_policy_version.threshold,
    )?;

    emit!(PaymentApprovalRecorded {
        organization: ctx.accounts.organization.key(),
        payment: ctx.accounts.payment.key(),
        approval: ctx.accounts.approval.key(),
        approver_member: ctx.accounts.approver_member.key(),
        authorization_wallet: ctx.accounts.approver_member.authorized_wallet,
        payment_revision: ctx.accounts.payment.payment_revision,
        member_authorization_revision: ctx.accounts.approver_member.authorization_revision,
        policy_version: ctx.accounts.approval_policy_version.key(),
        terms_hash: ctx.accounts.payment.terms_hash,
        approved_at,
    });

    if reservation_outcome.threshold_reached {
        emit!(PaymentApprovalThresholdReached {
            organization: ctx.accounts.organization.key(),
            payment: ctx.accounts.payment.key(),
            policy_version: ctx.accounts.approval_policy_version.key(),
            vault: ctx.accounts.vault_state.key(),
            triggered_by_member: ctx.accounts.approver_member.key(),
            valid_approval_count,
            threshold: ctx.accounts.approval_policy_version.threshold,
            amount: ctx.accounts.payment.amount,
            funds_reserved: reservation_outcome.funds_reserved,
            vault_reserved_total: ctx.accounts.vault_state.reserved_total,
            payment_state: ctx.accounts.payment.payment_state,
            processed_at: approved_at,
        });
    }

    Ok(())
}
