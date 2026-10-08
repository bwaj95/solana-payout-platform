mod common;

use common::{
    accounts::{payment, spl_token_account, vault_state},
    fixtures::{setup_execute_payment_fixture, ExecutePaymentFixture},
};

use solana_payout_platform::{PaymentState, ReservationState, ROLE_EXECUTOR};
use solana_pubkey::Pubkey;

fn assert_payment_is_still_approved(fixture: &ExecutePaymentFixture) {
    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Approved);

    assert_eq!(stored_payment.reservation_state, ReservationState::Held);
}

#[test]
fn test_execute_spl_payment_success() {
    let mut fixture = setup_execute_payment_fixture();

    let vault_before = spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata);

    let destination_before = spl_token_account(&fixture.approval.svm, &fixture.destination_ata);

    let vault_state_before = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    assert_eq!(
        vault_state_before.reserved_total,
        fixture.approval.payment_amount
    );

    fixture.execute().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);

    assert_eq!(stored_payment.reservation_state, ReservationState::Consumed);

    let stored_vault_state = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    assert_eq!(stored_vault_state.reserved_total, 0);

    let vault_after = spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata);

    let destination_after = spl_token_account(&fixture.approval.svm, &fixture.destination_ata);

    assert_eq!(
        vault_after.amount,
        vault_before.amount - fixture.approval.payment_amount
    );

    assert_eq!(
        destination_after.amount,
        destination_before.amount + fixture.approval.payment_amount
    );
}

#[test]
fn test_execute_before_execute_after_is_rejected() {
    let mut fixture = setup_execute_payment_fixture();

    fixture.set_execute_after(i64::MAX);

    let vault_balance_before =
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata).amount;

    fixture.execute().unwrap_err();

    assert_payment_is_still_approved(&fixture);

    let vault_balance_after =
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata).amount;

    assert_eq!(vault_balance_after, vault_balance_before);
}

#[test]
fn test_member_without_executor_role_is_rejected() {
    let mut fixture = setup_execute_payment_fixture();

    fixture.update_executor_member(|member| {
        member.roles &= !ROLE_EXECUTOR;
    });

    fixture.execute().unwrap_err();

    assert_payment_is_still_approved(&fixture);

    let destination = spl_token_account(&fixture.approval.svm, &fixture.destination_ata);

    assert_eq!(destination.amount, 0);
}

#[test]
fn test_paused_organization_rejects_execution() {
    let mut fixture = setup_execute_payment_fixture();

    fixture.update_organization(|organization| {
        organization.paused = true;
    });

    fixture.execute().unwrap_err();

    assert_payment_is_still_approved(&fixture);

    let destination = spl_token_account(&fixture.approval.svm, &fixture.destination_ata);

    assert_eq!(destination.amount, 0);
}

#[test]
fn test_recipient_wallet_rotation_rejects_execution() {
    let mut fixture = setup_execute_payment_fixture();

    let new_destination_wallet = Pubkey::new_unique();

    fixture.rotate_recipient_wallet(new_destination_wallet);

    fixture.execute().unwrap_err();

    /*
     * Payment approval remains held until an explicit cancellation or
     * invalidation instruction releases the reservation.
     */
    assert_payment_is_still_approved(&fixture);

    let new_destination = spl_token_account(&fixture.approval.svm, &fixture.destination_ata);

    assert_eq!(new_destination.amount, 0);
}

#[test]
fn test_vault_balance_below_reserved_total_is_rejected() {
    let mut fixture = setup_execute_payment_fixture();

    fixture
        .approval
        .set_vault_token_balance(fixture.approval.payment_amount - 1);

    fixture.execute().unwrap_err();

    assert_payment_is_still_approved(&fixture);

    let stored_vault = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    assert_eq!(stored_vault.reserved_total, fixture.approval.payment_amount);
}

#[test]
fn test_reservation_smaller_than_payment_is_rejected() {
    let mut fixture = setup_execute_payment_fixture();

    let invalid_reserved_total = fixture.approval.payment_amount - 1;

    fixture.approval.update_vault(|vault| {
        vault.reserved_total = invalid_reserved_total;
    });

    fixture.execute().unwrap_err();

    assert_payment_is_still_approved(&fixture);

    let stored_vault = vault_state(&fixture.approval.svm, &fixture.approval.vault_state);

    assert_eq!(stored_vault.reserved_total, invalid_reserved_total);
}

#[test]
fn test_payment_cannot_be_executed_twice() {
    let mut fixture = setup_execute_payment_fixture();

    fixture.execute().unwrap();

    let vault_balance_after_first_execution =
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata).amount;

    let destination_balance_after_first_execution =
        spl_token_account(&fixture.approval.svm, &fixture.destination_ata).amount;

    /*
     * The refreshed LiteSVM blockhash ensures this reaches the program.
     * It should fail because the payment is already Paid and its reservation
     * is already Consumed.
     */
    fixture.execute().unwrap_err();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);

    assert_eq!(stored_payment.reservation_state, ReservationState::Consumed);

    assert_eq!(
        spl_token_account(&fixture.approval.svm, &fixture.approval.vault_ata,).amount,
        vault_balance_after_first_execution
    );

    assert_eq!(
        spl_token_account(&fixture.approval.svm, &fixture.destination_ata,).amount,
        destination_balance_after_first_execution
    );
}

#[test]
fn test_disabling_old_policy_does_not_cancel_finalized_payment() {
    let mut fixture = setup_execute_payment_fixture();

    /*
     * Approved + Held is our final authorization checkpoint. Disabling the
     * old policy version does not retroactively cancel this payment.
     */
    fixture.approval.update_policy(|policy| {
        policy.enabled = false;
    });

    fixture.execute().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);

    assert_eq!(stored_payment.reservation_state, ReservationState::Consumed);
}

#[test]
fn test_approver_deactivation_does_not_cancel_finalized_payment() {
    let mut fixture = setup_execute_payment_fixture();

    /*
     * The approvals were valid when the payment reached Approved + Held.
     * Later member deactivation does not retroactively remove that completed
     * authorization decision.
     */
    fixture.approval.update_approver_member(0, |member| {
        member.active = false;
    });

    fixture.approval.update_approver_member(1, |member| {
        member.active = false;
    });

    fixture.execute().unwrap();

    let stored_payment = payment(&fixture.approval.svm, &fixture.approval.payment);

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);
}
