use anchor_lang::prelude::*;

use crate::{
    compute_payment_terms_hash, error::ErrorCode, ApprovalPolicyVersion, Asset, Member,
    Organization, Payment, PaymentState, Recipient, ReservationState, SettlementRail, VaultState,
    INITIAL_PAYMENT_REVISION, MAX_POLICY_MEMBERS, MEMBER_SEED, ORGANIZATION_SEED, PAYMENT_SEED,
    POLICY_SEED, RECIPIENT_SEED, ROLE_ADMIN, ROLE_PREPARER, VAULT_SEED,
};

#[derive(Accounts)]
#[instruction(
    payment_id: u64,
    amount: u64,
    execute_after: i64
)]
pub struct CreatePayment<'info> {
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
            preparer_member.member_id.to_le_bytes().as_ref(),
        ],
        bump = preparer_member.bump,
        has_one = organization
            @ ErrorCode::MemberOrganizationMismatch,
        constraint = preparer_member.active
            @ ErrorCode::InactiveMember,
        constraint = preparer_member.authorized_wallet == authority.key()
            @ ErrorCode::UnauthorizedWallet,
        constraint = (
            preparer_member.roles & (ROLE_ADMIN | ROLE_PREPARER)
        ) != 0
            @ ErrorCode::MissingPaymentCreatorRole,
    )]
    pub preparer_member: Account<'info, Member>,

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
    pub recipient: Account<'info, Recipient>,

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
    pub approval_policy_version: Account<'info, ApprovalPolicyVersion>,

    #[account(
        init,
        payer = authority,
        space = 8 + Payment::INIT_SPACE,
        seeds = [
            PAYMENT_SEED,
            organization.key().as_ref(),
            payment_id.to_le_bytes().as_ref(),
        ],
        bump,
    )]
    pub payment: Account<'info, Payment>,

    pub system_program: Program<'info, System>,
}

pub fn create_payment_handler(
    ctx: Context<CreatePayment>,
    payment_id: u64,
    amount: u64,
    execute_after: i64,
) -> Result<()> {
    require!(payment_id > 0, ErrorCode::InvalidPaymentId);

    require!(amount > 0, ErrorCode::InvalidPaymentAmount);

    // Zero means the payment is immediately due.
    // A negative Unix timestamp is invalid.
    require!(execute_after >= 0, ErrorCode::InvalidExecuteAfter);

    let policy = &ctx.accounts.approval_policy_version;
    let eligible_approver_count = policy.eligible_members.len();

    require!(eligible_approver_count > 0, ErrorCode::EmptyPolicyMembers);

    require!(
        eligible_approver_count <= MAX_POLICY_MEMBERS as usize,
        ErrorCode::TooManyPolicyMembers
    );

    require!(
        policy.threshold > 0 && policy.threshold as usize <= eligible_approver_count,
        ErrorCode::InvalidPolicyThreshold
    );

    // Public USDC POC: native SOL and private settlement are not
    // implemented by this instruction.
    let mint = match &ctx.accounts.vault_state.asset {
        Asset::Spl { mint } => *mint,
        Asset::NativeSol => {
            return err!(ErrorCode::UnsupportedVaultAsset);
        }
    };

    let organization = ctx.accounts.organization.key();
    let recipient = ctx.accounts.recipient.key();
    let destination = ctx.accounts.recipient.current_destination;
    let recipient_wallet_revision = ctx.accounts.recipient.wallet_revision;
    let vault = ctx.accounts.vault_state.key();
    let policy_version = ctx.accounts.approval_policy_version.key();

    let payment_revision = INITIAL_PAYMENT_REVISION;
    let settlement_rail = SettlementRail::PublicSpl;

    let terms_hash = compute_payment_terms_hash(
        &organization,
        payment_id,
        payment_revision,
        &recipient,
        &destination,
        recipient_wallet_revision,
        &vault,
        &mint,
        amount,
        &policy_version,
        settlement_rail,
        execute_after,
    );

    ctx.accounts.payment.set_inner(Payment {
        organization,
        payment_id,
        created_by: ctx.accounts.preparer_member.key(),
        recipient,
        destination,
        recipient_wallet_revision,
        vault,
        amount,
        policy_version,
        payment_revision,
        settlement_rail,
        execute_after,
        terms_hash,
        payment_state: PaymentState::PendingApproval,
        reservation_state: ReservationState::None,
        bump: ctx.bumps.payment,
    });

    Ok(())
}
