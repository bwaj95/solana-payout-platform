mod common;

use common::{
    accounts::{payment, spl_token_account, vault_state},
    fixtures::{setup_withdraw_spl_vault_funds_fixture, WithdrawSplVaultFundsFixture},
};

use solana_payout_platform::{PaymentState, ReservationState, ROLE_EXECUTOR, ROLE_TREASURY};
use solana_pubkey::Pubkey;

fn balances(fixture: &WithdrawSplVaultFundsFixture) -> (u64, u64, u64) {
    let vault_balance = spl_token_account(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_ata,
    )
    .amount;

    let destination_balance = spl_token_account(
        &fixture.execution.approval.svm,
        &fixture.destination_token_account,
    )
    .amount;

    let reserved_total = vault_state(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_state,
    )
    .reserved_total;

    (vault_balance, destination_balance, reserved_total)
}

#[test]
fn test_withdraw_spl_vault_funds_success() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let amount = 1_000_000;
    let before = balances(&fixture);

    fixture.withdraw(amount).unwrap();

    let after = balances(&fixture);

    assert_eq!(after.0, before.0 - amount);
    assert_eq!(after.1, before.1 + amount);

    // Withdrawal must never reduce payment reservations.
    assert_eq!(after.2, before.2);
}

#[test]
fn test_withdrawing_exact_unreserved_balance_succeeds() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let available = fixture.available_unreserved_balance();
    let reserved_before = balances(&fixture).2;

    fixture.withdraw(available).unwrap();

    let after = balances(&fixture);

    // Only the tokens reserved for the approved payment remain.
    assert_eq!(after.0, reserved_before);
    assert_eq!(after.1, available);
    assert_eq!(after.2, reserved_before);
}

#[test]
fn test_withdrawing_more_than_unreserved_balance_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let available = fixture.available_unreserved_balance();
    let before = balances(&fixture);

    fixture.withdraw(available + 1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_zero_withdrawal_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let before = balances(&fixture);

    fixture.withdraw(0).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_treasury_only_member_can_withdraw() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.update_treasury_member(|member| {
        member.roles = ROLE_TREASURY;
    });

    fixture.withdraw(1).unwrap();
}

#[test]
fn test_member_without_admin_or_treasury_role_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.update_treasury_member(|member| {
        member.roles = ROLE_EXECUTOR;
    });

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_unauthorized_wallet_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let before = balances(&fixture);

    fixture.withdraw_as_approver(0, 1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_paused_organization_rejects_withdrawal() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.update_organization(|organization| {
        organization.paused = true;
    });

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_inactive_vault_rejects_withdrawal() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.update_vault(|vault| {
        vault.active = false;
    });

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_vault_balance_below_reserved_total_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    let reserved_total = vault_state(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_state,
    )
    .reserved_total;

    fixture
        .execution
        .approval
        .set_vault_token_balance(reserved_total - 1);

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_noncanonical_destination_token_account_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.use_noncanonical_destination_token_account();

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_destination_with_wrong_token_owner_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.corrupt_destination_token_owner(Pubkey::new_unique());

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_wrong_mint_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.use_wrong_mint();

    fixture.withdraw(1).unwrap_err();
}

#[test]
fn test_default_destination_wallet_is_rejected() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.prepare_destination(Pubkey::default());

    let before = balances(&fixture);

    fixture.withdraw(1).unwrap_err();

    assert_eq!(balances(&fixture), before);
}

#[test]
fn test_vault_cannot_withdraw_to_itself() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    fixture.use_vault_as_destination();

    let vault_before = spl_token_account(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_ata,
    )
    .amount;

    fixture.withdraw(1).unwrap_err();

    let vault_after = spl_token_account(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_ata,
    )
    .amount;

    assert_eq!(vault_after, vault_before);
}

#[test]
fn test_withdrawal_preserves_tokens_needed_for_reserved_payment() {
    let mut fixture = setup_withdraw_spl_vault_funds_fixture();

    /*
     * Withdraw every unreserved token. The vault will then contain exactly
     * the amount held for the approved payment.
     */
    let available = fixture.available_unreserved_balance();

    fixture.withdraw(available).unwrap();

    let reserved_total = vault_state(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_state,
    )
    .reserved_total;

    let vault_after_withdrawal = spl_token_account(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.vault_ata,
    )
    .amount;

    assert_eq!(vault_after_withdrawal, reserved_total);

    /*
     * The reserved payment must still execute successfully. This proves that
     * withdrawal cannot consume funds already promised to approved payments.
     */
    fixture.execution.execute().unwrap();

    let stored_payment = payment(
        &fixture.execution.approval.svm,
        &fixture.execution.approval.payment,
    );

    assert_eq!(stored_payment.payment_state, PaymentState::Paid);
    assert_eq!(stored_payment.reservation_state, ReservationState::Consumed);

    assert_eq!(
        vault_state(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.vault_state,
        )
        .reserved_total,
        0
    );

    assert_eq!(
        spl_token_account(
            &fixture.execution.approval.svm,
            &fixture.execution.approval.vault_ata,
        )
        .amount,
        0
    );
}
