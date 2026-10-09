mod common;

use common::{
    accounts::{organization, payment},
    fixtures::setup_organization_pause_fixture,
};

use solana_payout_platform::{PaymentState, ReservationState, ROLE_ADMIN};

#[test]
fn test_admin_can_pause_organization() {
    let mut fixture = setup_organization_pause_fixture();

    let organization_before = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(!organization_before.paused);

    fixture.set_paused(true).unwrap();

    let organization_after = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(organization_after.paused);
}

#[test]
fn test_admin_can_unpause_organization() {
    let mut fixture = setup_organization_pause_fixture();

    fixture.set_paused(true).unwrap();

    assert!(
        organization(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.organization,
        )
        .paused
    );

    /*
     * The same instruction must remain callable while paused. Otherwise the
     * organization would become permanently locked.
     */
    fixture.set_paused(false).unwrap();

    assert!(
        !organization(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.organization,
        )
        .paused
    );
}

#[test]
fn test_setting_existing_pause_state_is_rejected() {
    let mut fixture = setup_organization_pause_fixture();

    /*
     * Organizations begin unpaused, so setting false again is a no-op.
     */
    fixture.set_paused(false).unwrap_err();

    let stored_organization = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(!stored_organization.paused);
}

#[test]
fn test_member_without_admin_role_cannot_pause() {
    let mut fixture = setup_organization_pause_fixture();

    fixture.update_admin_member(|member| {
        member.roles &= !ROLE_ADMIN;
    });

    fixture.set_paused(true).unwrap_err();

    let stored_organization = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(!stored_organization.paused);
}

#[test]
fn test_inactive_admin_cannot_pause() {
    let mut fixture = setup_organization_pause_fixture();

    fixture.update_admin_member(|member| {
        member.active = false;
    });

    fixture.set_paused(true).unwrap_err();

    let stored_organization = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(!stored_organization.paused);
}

#[test]
fn test_unauthorized_wallet_cannot_use_admin_member() {
    let mut fixture = setup_organization_pause_fixture();

    fixture.set_paused_as_approver(0, true).unwrap_err();

    let stored_organization = organization(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.organization,
    );

    assert!(!stored_organization.paused);
}

#[test]
fn test_pause_blocks_execution_and_unpause_restores_it() {
    let mut fixture = setup_organization_pause_fixture();

    /*
     * The fixture already contains an Approved + Held payment.
     */
    let payment_before = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(payment_before.payment_state, PaymentState::Approved);

    assert_eq!(payment_before.reservation_state, ReservationState::Held);

    fixture.set_paused(true).unwrap();

    /*
     * No transfer or state transition can occur while paused.
     */
    fixture.execution.execute().unwrap_err();

    let payment_while_paused = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(payment_while_paused.payment_state, PaymentState::Approved);

    assert_eq!(
        payment_while_paused.reservation_state,
        ReservationState::Held
    );

    /*
     * Unpausing should restore execution without requiring new approvals,
     * because pausing temporarily freezes rather than cancels payments.
     */
    fixture.set_paused(false).unwrap();

    fixture.execution.execute().unwrap();

    let payment_after_execution = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(payment_after_execution.payment_state, PaymentState::Paid);

    assert_eq!(
        payment_after_execution.reservation_state,
        ReservationState::Consumed
    );
}
