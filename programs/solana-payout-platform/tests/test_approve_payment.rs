mod common;

use common::{
    accounts::{approval, payment, vault_state},
    fixtures::setup_approval_fixture,
};
use solana_payout_platform::{Asset, PaymentState, ReservationState};

use solana_pubkey::Pubkey;

#[test]
fn test_first_approval_is_recorded_below_threshold() {
    let mut fixture = setup_approval_fixture();

    // Alice is the first approver, so there are no previous witnesses.
    fixture.approve(0, &[]).unwrap();

    let alice_approval_pda = fixture.approval_pda(0);
    let alice_approval = approval(&fixture.svm, &alice_approval_pda);

    assert_eq!(alice_approval.organization, fixture.organization);
    assert_eq!(alice_approval.payment, fixture.payment);
    assert_eq!(alice_approval.member, fixture.approvers[0].member);
    assert_eq!(alice_approval.payment_revision, fixture.payment_revision);
    assert_eq!(
        alice_approval.member_authorization_revision,
        fixture.approvers[0].authorization_revision
    );
    assert_eq!(alice_approval.policy, fixture.approval_policy_version);

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_threshold_approval_reserves_funds_immediately() {
    let mut fixture = setup_approval_fixture();

    // Alice creates the first approval.
    fixture.approve(0, &[]).unwrap();

    // Bob's transaction supplies Alice's Approval + Member pair.
    // Bob is counted directly by the typed approver_member account.
    fixture.approve(1, &[0]).unwrap();

    let bob_approval_pda = fixture.approval_pda(1);

    // Both immutable Approval accounts now exist.
    approval(&fixture.svm, &fixture.approval_pda(0));
    approval(&fixture.svm, &bob_approval_pda);

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Approved);
    assert_eq!(stored_payment.reservation_state, ReservationState::Held);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, fixture.payment_amount);
}

#[test]
fn test_threshold_with_insufficient_funds_moves_to_awaiting_funds() {
    let mut fixture = setup_approval_fixture();

    fixture.set_vault_token_balance(fixture.payment_amount - 1);

    fixture.approve(0, &[]).unwrap();
    fixture.approve(1, &[0]).unwrap();

    // Bob's approval is preserved even though reservation was impossible.
    approval(&fixture.svm, &fixture.approval_pda(1));

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::AwaitingFunds);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_finalizer_reserves_after_awaiting_funds_is_funded() {
    let mut fixture = setup_approval_fixture();

    fixture.set_vault_token_balance(fixture.payment_amount - 1);

    fixture.approve(0, &[]).unwrap();
    fixture.approve(1, &[0]).unwrap();

    let awaiting_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(awaiting_payment.payment_state, PaymentState::AwaitingFunds);

    // The vault is funded later.
    fixture.set_vault_token_balance(fixture.payment_amount * 2);

    // Recovery finalization supplies both existing approvals.
    fixture.finalize(&[0, 1]).unwrap();

    let approved_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(approved_payment.payment_state, PaymentState::Approved);
    assert_eq!(approved_payment.reservation_state, ReservationState::Held);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, fixture.payment_amount);
}

#[test]
fn test_finalizer_recovers_approvals_created_without_witnesses() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    /*
     * Simulate stale RPC/concurrent discovery:
     *
     * Bob's transaction does not know that Alice's approval exists, so it
     * records Bob but cannot see the threshold.
     */
    fixture.approve(1, &[]).unwrap();

    let pending_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(pending_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(pending_payment.reservation_state, ReservationState::None);

    // The backend later discovers both approvals and recovers the transition.
    fixture.finalize(&[0, 1]).unwrap();

    let approved_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(approved_payment.payment_state, PaymentState::Approved);
    assert_eq!(approved_payment.reservation_state, ReservationState::Held);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, fixture.payment_amount);
}

#[test]
fn test_inactive_current_approver_is_rejected() {
    let mut fixture = setup_approval_fixture();

    fixture.update_approver_member(0, |member| {
        member.active = false;
    });

    fixture.approve(0, &[]).unwrap_err();

    assert!(fixture.svm.get_account(&fixture.approval_pda(0)).is_none());
}

#[test]
fn test_member_not_in_policy_is_rejected() {
    let mut fixture = setup_approval_fixture();
    let alice_member = fixture.approvers[0].member;

    fixture.update_policy(|policy| {
        policy
            .eligible_members
            .retain(|member| *member != alice_member);
    });

    fixture.approve(0, &[]).unwrap_err();

    assert!(fixture.svm.get_account(&fixture.approval_pda(0)).is_none());
}

#[test]
fn test_disabled_policy_is_rejected() {
    let mut fixture = setup_approval_fixture();

    fixture.update_policy(|policy| {
        policy.enabled = false;
    });

    fixture.approve(0, &[]).unwrap_err();
}

#[test]
fn test_recipient_rotation_invalidates_payment_approval() {
    let mut fixture = setup_approval_fixture();

    fixture.update_recipient(|recipient| {
        recipient.current_destination = Pubkey::new_unique();
        recipient.wallet_revision += 1;
    });

    fixture.approve(0, &[]).unwrap_err();
}

#[test]
fn test_modified_payment_terms_are_rejected() {
    let mut fixture = setup_approval_fixture();

    /*
     * The amount is changed without recomputing terms_hash.
     * validate_payment_terms must detect this.
     */
    fixture.update_payment(|payment| {
        payment.amount += 1;
    });

    fixture.approve(0, &[]).unwrap_err();
}

#[test]
fn test_native_sol_vault_is_rejected_for_public_spl_payment() {
    let mut fixture = setup_approval_fixture();

    fixture.update_vault(|vault| {
        vault.asset = Asset::NativeSol;
    });

    fixture.approve(0, &[]).unwrap_err();
}

#[test]
fn test_inactive_previous_approver_is_not_counted() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    // Alice was valid when she approved but is deactivated afterward.
    fixture.update_approver_member(0, |member| {
        member.active = false;
    });

    // Alice's old Approval is supplied, but must not count.
    fixture.approve(1, &[0]).unwrap();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);

    // Bob's valid approval was still recorded.
    approval(&fixture.svm, &fixture.approval_pda(1));
}

#[test]
fn test_wallet_rotated_previous_approval_is_not_counted() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    /*
     * Alice's old Approval stores authorization revision 1 and her old wallet.
     * The Member now contains revision 2 and a different wallet.
     */
    fixture.update_approver_member(0, |member| {
        member.authorization_revision += 1;
        member.authorized_wallet = Pubkey::new_unique();
    });

    fixture.approve(1, &[0]).unwrap();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_finalizer_rejects_threshold_not_reached() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    fixture.finalize(&[0]).unwrap_err();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);
}

#[test]
fn test_duplicate_member_witness_is_rejected() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    /*
     * Supplying Alice twice must not make a 2-of-3 threshold appear reached.
     */
    fixture.finalize(&[0, 0]).unwrap_err();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_approver_cannot_create_same_approval_twice() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    /*
     * Including a witness changes the transaction message, preventing the
     * second submission from merely being rejected as a duplicate signature.
     *
     * The named Approval PDA is already initialized, so Anchor's init
     * constraint must reject the second attempt.
     */
    fixture.approve(0, &[0]).unwrap_err();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_mismatched_approval_and_member_pair_is_rejected() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    let alice_approval = fixture.approval_pda(0);
    let bob_member = fixture.approvers[1].member;

    /*
     * Alice's Approval is deliberately paired with Bob's Member PDA.
     */
    let malformed_witnesses = vec![(alice_approval, bob_member)];

    fixture
        .finalize_with_witnesses(&malformed_witnesses)
        .unwrap_err();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
}

#[test]
fn test_tampered_approval_terms_hash_is_rejected() {
    let mut fixture = setup_approval_fixture();

    // Create both approvals without supplying previous witnesses.
    fixture.approve(0, &[]).unwrap();
    fixture.approve(1, &[]).unwrap();

    fixture.update_approval(0, |approval| {
        approval.terms_hash = [7u8; 32];
    });

    fixture.finalize(&[0, 1]).unwrap_err();

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);

    let stored_vault = vault_state(&fixture.svm, &fixture.vault_state);

    assert_eq!(stored_vault.reserved_total, 0);
}

#[test]
fn test_vault_reservation_invariant_violation_is_rejected() {
    let mut fixture = setup_approval_fixture();

    fixture.approve(0, &[]).unwrap();

    /*
     * Simulate corrupted vault accounting where reserved_total exceeds the
     * actual token balance. checked_sub must return an error instead of
     * underflowing.
     */
    fixture.update_vault(|vault| {
        vault.reserved_total = u64::MAX;
    });

    let bob_approval_pda = fixture.approval_pda(1);

    fixture.approve(1, &[0]).unwrap_err();

    /*
     * Because the instruction failed atomically, Bob's newly initialized
     * Approval account must also have been rolled back.
     */
    assert!(fixture.svm.get_account(&bob_approval_pda).is_none());

    let stored_payment = payment(&fixture.svm, &fixture.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(stored_payment.reservation_state, ReservationState::None);
}
