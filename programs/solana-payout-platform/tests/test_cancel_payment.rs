mod common;

use common::{
    accounts::{payment, spl_token_account, vault_state},
    fixtures::setup_cancel_payment_fixture,
};

use solana_payout_platform::{PaymentState, ReservationState, ROLE_ADMIN};
use solana_pubkey::Pubkey;

#[test]
fn test_cancel_pending_payment_without_releasing_other_reservations() {
    let mut fixture = setup_cancel_payment_fixture();

    /*
     * This payment has no reservation, but the vault may contain
     * reservations belonging to other payments.
     */
    let other_payments_reserved_total = 3_000_000;

    fixture.approval.update_vault(|vault| {
        vault.reserved_total = other_payments_reserved_total;
    });

    let revision_before =
        payment(&fixture.approval.svm, &fixture.approval.payment).payment_revision;

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    /*
     * Terminal cancellation does not change the payment revision.
     */
    assert_eq!(stored_payment.payment_revision, revision_before);

    let stored_vault = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    assert_eq!(stored_vault.reserved_total, other_payments_reserved_total);
}

#[test]
fn test_cancel_awaiting_funds_payment_releases_nothing() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_awaiting_funds();

    /*
     * Simulate reservations held by unrelated payments.
     */
    let other_payments_reserved_total = 1_000_000;

    fixture.approval.update_vault(|vault| {
        vault.reserved_total = other_payments_reserved_total;
    });

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::None);

    assert_eq!(
        vault_state(&fixture.approval.svm, &fixture.approval.vault_state,).reserved_total,
        other_payments_reserved_total
    );
}

#[test]
fn test_cancel_approved_payment_releases_only_its_reservation() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_approved_and_held();

    let another_payment_reservation = 3_000_000;

    fixture.approval.update_vault(|vault| {
        vault.reserved_total += another_payment_reservation;
    });

    let vault_token_balance_before =
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata).amount;

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::Released);

    let stored_vault = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    /*
     * Only this payment's amount was released. The unrelated reservation
     * remains protected.
     */
    assert_eq!(stored_vault.reserved_total, another_payment_reservation);

    /*
     * Cancellation changes accounting only. It does not transfer tokens.
     */
    let vault_token_balance_after =
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata).amount;

    assert_eq!(vault_token_balance_after, vault_token_balance_before);
}

#[test]
fn test_cancel_approved_payment_while_organization_is_paused() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_approved_and_held();

    fixture.update_organization(|organization| {
        organization.paused = true;
    });

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::Released);

    assert_eq!(
        vault_state(&fixture.approval.svm, &fixture.approval.vault_state,).reserved_total,
        0
    );
}

#[test]
fn test_cancel_approved_payment_while_vault_is_inactive() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_approved_and_held();

    fixture.approval.update_vault(|vault| {
        vault.active = false;
    });

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::Released);

    assert_eq!(
        vault_state(&fixture.approval.svm, &fixture.approval.vault_state,).reserved_total,
        0
    );
}

#[test]
fn test_recipient_rotation_does_not_block_cancellation() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_approved_and_held();

    fixture.approval.update_recipient(|recipient| {
        recipient.current_destination = Pubkey::new_unique();
        recipient.wallet_revision += 1;
    });

    fixture.cancel().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::Released);

    assert_eq!(
        vault_state(&fixture.approval.svm, &fixture.approval.vault_state,).reserved_total,
        0
    );
}

#[test]
fn test_paid_payment_cannot_be_cancelled() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.approval.update_payment(|payment| {
        payment.payment_state = PaymentState::Paid;
        payment.reservation_state = ReservationState::Consumed;
    });

    fixture.cancel().unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);

    assert_eq!(stored_payment.reservation_state, ReservationState::Consumed);
}

#[test]
fn test_payment_cannot_be_cancelled_twice() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.cancel().unwrap();
    fixture.cancel().unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(stored_payment.reservation_state, ReservationState::None);
}

#[test]
fn test_non_admin_member_cannot_cancel_payment() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.update_admin_member(|member| {
        member.roles &= !ROLE_ADMIN;
    });

    fixture.cancel().unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
}

#[test]
fn test_unauthorized_wallet_cannot_use_admin_member() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.cancel_as_approver(0).unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);
}

#[test]
fn test_invalid_payment_and_reservation_combination_is_rejected() {
    let mut fixture = setup_cancel_payment_fixture();

    let payment_amount = fixture.approval.payment_amount;

    fixture.approval.update_payment(|payment| {
        payment.reservation_state = ReservationState::Held;
    });

    fixture.approval.update_vault(|vault| {
        vault.reserved_total = payment_amount;
    });

    fixture.cancel().unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::PendingApproval);

    assert_eq!(stored_payment.reservation_state, ReservationState::Held);
}

#[test]
fn test_reservation_underflow_rejects_cancellation_atomically() {
    let mut fixture = setup_cancel_payment_fixture();

    fixture.reach_approved_and_held();

    let invalid_reserved_total = fixture.approval.payment_amount - 1;

    fixture.approval.update_vault(|vault| {
        vault.reserved_total = invalid_reserved_total;
    });

    fixture.cancel().unwrap_err();

    /*
     * The payment remains executable/cancellable after the accounting
     * invariant is repaired. The failed cancellation changed nothing.
     */
    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Approved);

    assert_eq!(stored_payment.reservation_state, ReservationState::Held);

    assert_eq!(
        vault_state(&fixture.approval.svm, &fixture.approval.vault_state,).reserved_total,
        invalid_reserved_total
    );
}
