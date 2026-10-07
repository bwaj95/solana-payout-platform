use anchor_lang::prelude::*;

use crate::{
    compute_payment_terms_hash, error::ErrorCode, Approval, ApprovalPolicyVersion, Asset, Member,
    Payment, PaymentState, ReservationState, SettlementRail, VaultState, APPROVAL_SEED,
    MAX_POLICY_MEMBERS, MEMBER_SEED, ROLE_APPROVER,
};

pub(crate) struct ReservationOutcome {
    pub threshold_reached: bool,
    pub funds_reserved: bool,
}

/// Defensively revalidate the policy whenever it is used.
///
/// create_policy_version already prevents malformed policies, but repeating
/// these inexpensive invariants keeps the payment instructions self-contained.
pub(crate) fn validate_policy(policy: &ApprovalPolicyVersion) -> Result<()> {
    require!(
        !policy.eligible_members.is_empty(),
        ErrorCode::EmptyPolicyMembers
    );

    require!(
        policy.eligible_members.len() <= MAX_POLICY_MEMBERS as usize,
        ErrorCode::TooManyPolicyMembers
    );

    require!(
        policy.threshold > 0 && policy.threshold as usize <= policy.eligible_members.len(),
        ErrorCode::InvalidPolicyThreshold
    );

    Ok(())
}

/// Extract the SPL mint from the vault and recompute the payment terms hash.
///
/// This detects unexpected changes to the frozen Payment fields before an
/// approval is accepted or funds are reserved.
pub(crate) fn validate_payment_terms(
    organization: Pubkey,
    payment: &Payment,
    vault_mint: &Pubkey,
) -> Result<()> {
    require!(
        payment.settlement_rail == SettlementRail::PublicSpl,
        ErrorCode::UnsupportedSettlementRail
    );

    let expected_terms_hash = compute_payment_terms_hash(
        &organization,
        payment.payment_id,
        payment.payment_revision,
        &payment.recipient,
        &payment.destination,
        payment.recipient_wallet_revision,
        &payment.vault,
        &vault_mint,
        payment.amount,
        &payment.policy_version,
        payment.settlement_rail,
        payment.execute_after,
    );

    require!(
        expected_terms_hash == payment.terms_hash,
        ErrorCode::PaymentTermsMismatch
    );

    Ok(())
}

/// Validate Approval + Member pairs supplied through remaining_accounts.
///
/// Expected order:
///
/// approval_1, member_1, approval_2, member_2, ...
///
/// The client discovers these accounts, but this function treats them as
/// untrusted and independently validates every relationship.
///
/// `initially_seen_member` is Some(current_approver) during approve_payment.
/// This prevents the caller from passing an older approval belonging to the
/// same member and counting the member twice.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_and_count_approval_witnesses(
    remaining_accounts: &[AccountInfo<'_>],
    organization: Pubkey,
    payment_key: Pubkey,
    payment: &Payment,
    policy_key: Pubkey,
    policy: &ApprovalPolicyVersion,
    initially_seen_member: Option<Pubkey>,
) -> Result<u8> {
    require!(
        remaining_accounts.len() % 2 == 0,
        ErrorCode::InvalidRemainingApprovalAccounts
    );

    let pair_count = remaining_accounts.len() / 2;

    require!(
        pair_count <= MAX_POLICY_MEMBERS as usize,
        ErrorCode::TooManyApprovalAccounts
    );

    let mut seen_members: Vec<Pubkey> = Vec::with_capacity(pair_count + 1);

    if let Some(member) = initially_seen_member {
        seen_members.push(member);
    }

    let mut valid_approval_count = 0u8;

    for pair in remaining_accounts.chunks_exact(2) {
        let approval_info = &pair[0];
        let member_info = &pair[1];

        /*
         * remaining_accounts are raw AccountInfo values, so Anchor does not
         * automatically enforce their owners or deserialize their data.
         */
        require_keys_eq!(
            *approval_info.owner,
            crate::ID,
            ErrorCode::InvalidApprovalAccount
        );

        require_keys_eq!(
            *member_info.owner,
            crate::ID,
            ErrorCode::InvalidMemberAccount
        );

        /*
         * Keep the Ref returned by try_borrow_data alive while the byte slice
         * is being deserialized. Approval and Member are then owned Rust values.
         */
        let approval = {
            let account_data = approval_info.try_borrow_data()?;
            let mut data_slice: &[u8] = &account_data;

            Approval::try_deserialize(&mut data_slice)?
        };

        let member = {
            let account_data = member_info.try_borrow_data()?;
            let mut data_slice: &[u8] = &account_data;

            Member::try_deserialize(&mut data_slice)?
        };

        let member_key = member_info.key();
        let approval_key = approval_info.key();

        /*
         * Validate the canonical Member PDA.
         */
        let member_id_bytes = member.member_id.to_le_bytes();

        let (expected_member, expected_member_bump) = Pubkey::find_program_address(
            &[MEMBER_SEED, organization.as_ref(), member_id_bytes.as_ref()],
            &crate::ID,
        );

        require_keys_eq!(expected_member, member_key, ErrorCode::InvalidMemberAccount);

        require_eq!(
            expected_member_bump,
            member.bump,
            ErrorCode::InvalidMemberAccount
        );

        require_keys_eq!(
            member.organization,
            organization,
            ErrorCode::MemberOrganizationMismatch
        );

        require!(
            policy.eligible_members.contains(&member_key),
            ErrorCode::ApproverNotEligibleForPolicy
        );

        /*
         * Validate the canonical Approval PDA.
         *
         * We derive using the revision that was recorded in the Approval.
         * Later, we separately compare it against the Member's current
         * authorization revision to decide whether it remains valid.
         */
        let payment_id_bytes = payment.payment_id.to_le_bytes();
        let payment_revision_bytes = payment.payment_revision.to_le_bytes();
        let approval_member_revision_bytes = approval.member_authorization_revision.to_le_bytes();

        let (expected_approval, expected_approval_bump) = Pubkey::find_program_address(
            &[
                APPROVAL_SEED,
                organization.as_ref(),
                payment_id_bytes.as_ref(),
                payment_revision_bytes.as_ref(),
                member_id_bytes.as_ref(),
                approval_member_revision_bytes.as_ref(),
            ],
            &crate::ID,
        );

        require_keys_eq!(
            expected_approval,
            approval_key,
            ErrorCode::InvalidApprovalAccount
        );

        require_eq!(
            expected_approval_bump,
            approval.bump,
            ErrorCode::InvalidApprovalAccount
        );

        /*
         * Validate that this Approval belongs to the exact payment terms that
         * we are currently evaluating.
         */

        require_keys_eq!(
            approval.organization,
            organization,
            ErrorCode::InvalidApprovalAccount
        );

        require_keys_eq!(
            approval.payment,
            payment_key,
            ErrorCode::InvalidApprovalAccount
        );

        require_eq!(
            approval.payment_revision,
            payment.payment_revision,
            ErrorCode::InvalidApprovalAccount
        );

        require_keys_eq!(
            approval.policy,
            policy_key,
            ErrorCode::InvalidApprovalAccount
        );

        require_keys_eq!(
            approval.member,
            member_key,
            ErrorCode::InvalidApprovalAccount
        );

        require!(
            approval.terms_hash == payment.terms_hash,
            ErrorCode::PaymentTermsMismatch
        );

        /*
         * A member may appear only once, even if the caller tries to supply
         * multiple Approval accounts for different wallet revisions.
         */
        require!(
            !seen_members.contains(&member_key),
            ErrorCode::DuplicateApprovalMember
        );

        seen_members.push(member_key);

        /*
         * These conditions can legitimately become false after approval:
         *
         * - member deactivated;
         * - approver role removed;
         * - authorized wallet rotated.
         *
         * Such an approval is structurally real, but is no longer counted.
         */
        let approval_is_currently_valid = member.active
            && (member.roles & ROLE_APPROVER) != 0
            && member.authorization_revision == approval.member_authorization_revision
            && member.authorized_wallet == approval.authorization_wallet;

        if !approval_is_currently_valid {
            continue;
        }

        valid_approval_count = valid_approval_count
            .checked_add(1)
            .ok_or(error!(ErrorCode::ArithmeticOverflow))?;
    }

    Ok(valid_approval_count)
}

/// If the threshold is visible and valid, attempt to reserve the payment.
///
/// Insufficient liquidity is not treated as an instruction error. We preserve
/// the approvals and move the payment into AwaitingFunds so finalization can be
/// retried after the vault is funded.
pub(crate) fn process_threshold_and_reservation(
    payment: &mut Payment,
    vault_state: &mut VaultState,
    vault_token_balance: u64,
    valid_approval_count: u8,
    threshold: u8,
) -> Result<ReservationOutcome> {
    if valid_approval_count < threshold {
        return Ok(ReservationOutcome {
            threshold_reached: false,
            funds_reserved: false,
        });
    }

    /*
     * Never write:
     *
     * vault_token_balance - reserved_total
     *
     * without checked_sub. If reserved_total somehow exceeds the real token
     * balance, ordinary subtraction would underflow.
     */
    let available_funds = vault_token_balance
        .checked_sub(vault_state.reserved_total)
        .ok_or(error!(ErrorCode::VaultReservationInvariantViolation))?;

    if available_funds < payment.amount {
        payment.payment_state = PaymentState::AwaitingFunds;
        payment.reservation_state = ReservationState::None;

        return Ok(ReservationOutcome {
            threshold_reached: true,
            funds_reserved: false,
        });
    }

    vault_state.reserved_total = vault_state
        .reserved_total
        .checked_add(payment.amount)
        .ok_or(error!(ErrorCode::ArithmeticOverflow))?;

    payment.payment_state = PaymentState::Approved;
    payment.reservation_state = ReservationState::Held;

    Ok(ReservationOutcome {
        threshold_reached: true,
        funds_reserved: true,
    })
}
