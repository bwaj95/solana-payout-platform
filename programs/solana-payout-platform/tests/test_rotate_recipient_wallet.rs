mod common;

use common::{
    accounts::{payment, recipient, vault_state},
    fixtures::setup_rotate_recipient_wallet_fixture,
};

use solana_payout_platform::{PaymentState, ReservationState, ROLE_ADMIN};
use solana_pubkey::Pubkey;

#[test]
fn test_rotate_recipient_wallet_success() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    let recipient_before = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    let payment_before = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    fixture.rotate().unwrap();

    let recipient_after = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(
        recipient_after.current_destination,
        fixture.new_destination_wallet
    );

    assert_eq!(
        recipient_after.wallet_revision,
        recipient_before.wallet_revision + 1
    );

    /*
     * Rotation does not silently rewrite an existing payment snapshot.
     */
    let payment_after = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(payment_after.destination, payment_before.destination);

    assert_eq!(
        payment_after.recipient_wallet_revision,
        payment_before.recipient_wallet_revision
    );

    assert_ne!(
        payment_after.destination,
        recipient_after.current_destination
    );

    assert_ne!(
        payment_after.recipient_wallet_revision,
        recipient_after.wallet_revision
    );
}

#[test]
fn test_same_destination_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    let current_destination = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    )
    .current_destination;

    fixture.prepare_destination(current_destination);

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(stored_recipient.current_destination, current_destination);
}

#[test]
fn test_default_destination_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    let recipient_before = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    fixture.prepare_destination(Pubkey::default());

    fixture.rotate().unwrap_err();

    let recipient_after = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(
        recipient_after.current_destination,
        recipient_before.current_destination
    );

    assert_eq!(
        recipient_after.wallet_revision,
        recipient_before.wallet_revision
    );
}

#[test]
fn test_noncanonical_destination_token_account_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.use_noncanonical_destination_token_account();

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_ne!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_destination_token_account_with_wrong_owner_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.corrupt_destination_token_owner(Pubkey::new_unique());

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_ne!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_mint_not_configured_in_vault_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.use_wrong_mint();

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_ne!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_unauthorized_wallet_cannot_rotate_recipient() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.rotate_as_approver(0).unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_ne!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_member_without_registrar_role_is_rejected() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.update_registrar_member(|member| {
        member.roles &= !ROLE_ADMIN;
    });

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_ne!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_rotation_is_allowed_while_organization_is_paused() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.update_organization(|organization| {
        organization.paused = true;
    });

    fixture.rotate().unwrap();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_rotation_is_allowed_while_vault_is_inactive() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.execution.approval.update_vault(|vault| {
        vault.active = false;
    });

    fixture.rotate().unwrap();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );
}

#[test]
fn test_wallet_revision_overflow_is_rejected_atomically() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    let original_destination = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    )
    .current_destination;

    fixture.execution.approval.update_recipient(|recipient| {
        recipient.wallet_revision = u32::MAX;
    });

    fixture.rotate().unwrap_err();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(stored_recipient.current_destination, original_destination);

    assert_eq!(stored_recipient.wallet_revision, u32::MAX);
}

#[test]
fn test_inactive_recipient_can_be_rotated_for_recovery() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.execution.approval.update_recipient(|recipient| {
        recipient.active = false;
    });

    fixture.rotate().unwrap();

    let stored_recipient = recipient(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.recipient,
    );

    assert_eq!(
        stored_recipient.current_destination,
        fixture.new_destination_wallet
    );

    /*
     * Rotation changes the destination but does not reactivate the recipient.
     */
    assert!(!stored_recipient.active);
}

#[test]
fn test_rotation_invalidates_execution_then_cancellation_releases_reservation() {
    let mut fixture = setup_rotate_recipient_wallet_fixture();

    fixture.rotate().unwrap();

    /*
     * The client supplies the new wallet's valid ATA. Execution must still
     * fail because the approved Payment contains the previous destination
     * and previous wallet revision.
     */
    fixture
        .execute_existing_payment_against_new_destination()
        .unwrap_err();

    let payment_after_failed_execution = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(
        payment_after_failed_execution.payment_state,
        PaymentState::Approved
    );

    assert_eq!(
        payment_after_failed_execution.reservation_state,
        ReservationState::Held
    );

    assert_eq!(
        vault_state(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.vault_state,
        )
        .reserved_total,
        fixture.execution.approval.payment_amount
    );

    /*
     * Cancellation does not require the recipient snapshot to match, so it
     * can release the now-unexecutable payment safely.
     */
    fixture.cancel_existing_payment().unwrap();

    let cancelled_payment = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(cancelled_payment.payment_state, PaymentState::Cancelled);

    assert_eq!(
        cancelled_payment.reservation_state,
        ReservationState::Released
    );

    assert_eq!(
        vault_state(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.vault_state,
        )
        .reserved_total,
        0
    );
}
