mod common;

use common::{
    accounts::payment, executor::create_payment, fixtures::setup_create_payment_fixture,
    pda::find_payment_pda,
};
use solana_payout_platform::{
    Asset, PaymentState, ReservationState, SettlementRail, ID, INITIAL_PAYMENT_REVISION,
    MAX_POLICY_MEMBERS, ROLE_ADMIN, ROLE_APPROVER,
};
use solana_pubkey::Pubkey;

#[test]
fn test_create_payment_success() {
    let mut fixture = setup_create_payment_fixture();
    let expected_terms_hash = fixture.expected_terms_hash();

    create_payment(
        &ID,
        &mut fixture.svm,
        &fixture.authority,
        &fixture.organization,
        &fixture.preparer_member,
        &fixture.recipient,
        &fixture.vault,
        &fixture.approval_policy_version,
        fixture.payment_id,
        fixture.amount,
        fixture.execute_after,
    )
    .unwrap();

    let (payment_pda, payment_bump) =
        find_payment_pda(&ID, &fixture.organization, fixture.payment_id);

    let created_payment = payment(&fixture.svm, &payment_pda);

    assert_eq!(created_payment.organization, fixture.organization);
    assert_eq!(created_payment.payment_id, fixture.payment_id);
    assert_eq!(created_payment.created_by, fixture.preparer_member);

    assert_eq!(created_payment.recipient, fixture.recipient);
    assert_eq!(created_payment.destination, fixture.destination);
    assert_eq!(
        created_payment.recipient_wallet_revision,
        fixture.recipient_wallet_revision
    );

    assert_eq!(created_payment.vault, fixture.vault);
    assert_eq!(created_payment.amount, fixture.amount);
    assert_eq!(
        created_payment.policy_version,
        fixture.approval_policy_version
    );

    assert_eq!(created_payment.payment_revision, INITIAL_PAYMENT_REVISION);
    assert_eq!(created_payment.settlement_rail, SettlementRail::PublicSpl);
    assert_eq!(created_payment.execute_after, fixture.execute_after);
    assert_eq!(created_payment.terms_hash, expected_terms_hash);

    assert_eq!(created_payment.payment_state, PaymentState::PendingApproval);
    assert_eq!(created_payment.reservation_state, ReservationState::None);

    assert_eq!(created_payment.bump, payment_bump);
}

#[test]
fn test_create_payment_admin_can_create_payment() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_preparer_member(|member| {
        member.roles = ROLE_ADMIN;
    });

    fixture.submit_create_payment().unwrap();
}

#[test]
fn test_create_payment_rejects_zero_payment_id() {
    let mut fixture = setup_create_payment_fixture();

    fixture.payment_id = 0;

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_zero_amount() {
    let mut fixture = setup_create_payment_fixture();

    fixture.amount = 0;

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_negative_execute_after() {
    let mut fixture = setup_create_payment_fixture();

    fixture.execute_after = -1;

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_paused_organization() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_organization(|organization| {
        organization.paused = true;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_inactive_preparer() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_preparer_member(|member| {
        member.active = false;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_unauthorized_wallet() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_preparer_member(|member| {
        member.authorized_wallet = Pubkey::new_unique();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_member_without_admin_or_preparer_role() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_preparer_member(|member| {
        member.roles = ROLE_APPROVER;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_member_from_another_organization() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_preparer_member(|member| {
        member.organization = Pubkey::new_unique();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_inactive_recipient() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_recipient(|recipient| {
        recipient.active = false;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_recipient_from_another_organization() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_recipient(|recipient| {
        recipient.organization = Pubkey::new_unique();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_inactive_vault() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_vault(|vault| {
        vault.active = false;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_vault_from_another_organization() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_vault(|vault| {
        vault.organization = Pubkey::new_unique();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_native_sol_vault() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_vault(|vault| {
        vault.asset = Asset::NativeSol;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_disabled_policy() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_policy(|policy| {
        policy.enabled = false;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_policy_from_another_organization() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_policy(|policy| {
        policy.organization = Pubkey::new_unique();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_policy_with_no_eligible_members() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_policy(|policy| {
        policy.eligible_members.clear();
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_zero_policy_threshold() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_policy(|policy| {
        policy.threshold = 0;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_threshold_above_member_count() {
    let mut fixture = setup_create_payment_fixture();
    let preparer_member = fixture.preparer_member;

    fixture.update_policy(|policy| {
        policy.threshold = 2;
        policy.eligible_members = vec![preparer_member];
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_too_many_policy_members() {
    let mut fixture = setup_create_payment_fixture();

    fixture.update_policy(|policy| {
        policy.eligible_members = vec![Pubkey::new_unique(); MAX_POLICY_MEMBERS as usize + 1];

        policy.threshold = 1;
    });

    fixture.submit_create_payment().unwrap_err();
}

#[test]
fn test_create_payment_rejects_duplicate_payment_id() {
    let mut fixture = setup_create_payment_fixture();

    fixture.submit_create_payment().unwrap();

    // Change the instruction data so this produces a different transaction
    // signature instead of LiteSVM rejecting it as the same transaction.
    fixture.execute_after = 1;

    fixture.submit_create_payment().unwrap_err();
}
