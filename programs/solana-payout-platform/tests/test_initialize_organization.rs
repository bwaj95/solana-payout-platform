mod common;

use solana_payout_platform::{self, INITIAL_OWNER_ROLES};

use crate::common::{
    accounts::{member, organization},
    executor::{execute_transaction, initialize_organization},
    fixtures::setup_test_context,
    instructions::initialize_organization_ix_with_accounts,
    pda::{find_member_pda, find_organization_pda},
    users::User,
};

#[test]
fn test_initialize_organization_success() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users) = setup_test_context(&program_id);

    let organization_id: u64 = 1;
    let creator: &User = &users["creator"];
    let owner_member_id: u64 = creator.id();

    initialize_organization(
        &program_id,
        &mut svm,
        creator,
        organization_id,
        owner_member_id,
    )
    .unwrap();

    let (organization_pda, organization_bump) =
        find_organization_pda(&program_id, &creator.pubkey(), organization_id);
    let (owner_member_pda, owner_member_bump) =
        find_member_pda(&program_id, &organization_pda, owner_member_id);

    let organization = organization(&svm, &organization_pda);
    let owner_member = member(&svm, &owner_member_pda);

    // Organization Assertions
    assert_eq!(organization.organization_id, organization_id);
    assert_eq!(organization.owner_member, owner_member_pda);
    assert_eq!(organization.creator, creator.pubkey());
    assert!(!organization.paused);
    assert_eq!(organization.bump, organization_bump);

    // Member Assertions
    assert_eq!(owner_member.organization, organization_pda);
    assert_eq!(owner_member.member_id, owner_member_id);
    assert!(owner_member.active);
    assert_eq!(owner_member.authorized_wallet, creator.pubkey());
    assert_eq!(owner_member.authorization_revision, 1);
    assert_eq!(owner_member.roles, INITIAL_OWNER_ROLES);
    assert_eq!(owner_member.bump, owner_member_bump);
}

#[test]
fn test_initialize_organization_rejects_duplicate() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator: &User = &users["creator"];
    let organization_id: u64 = 1;
    let owner_member_id: u64 = 1;

    initialize_organization(
        &program_id,
        &mut svm,
        creator,
        organization_id,
        owner_member_id,
    )
    .unwrap();

    let (organization_pda, _) =
        find_organization_pda(&program_id, &creator.pubkey(), organization_id);

    let (owner_member_pda, _) = find_member_pda(&program_id, &organization_pda, owner_member_id);

    let organization_before = svm
        .get_account(&organization_pda)
        .expect("Organization should exist");

    let owner_member_before = svm
        .get_account(&owner_member_pda)
        .expect("Owner Member should exist");

    // Produce a new blockhash so the second submission has a different
    // transaction signature and is not rejected merely as AlreadyProcessed.
    svm.expire_blockhash();

    let duplicate_result = initialize_organization(
        &program_id,
        &mut svm,
        creator,
        organization_id,
        owner_member_id,
    );

    assert!(
        duplicate_result.is_err(),
        "Duplicate initialization must fail"
    );

    let organization_after = svm
        .get_account(&organization_pda)
        .expect("Existing Organization must remain");

    let owner_member_after = svm
        .get_account(&owner_member_pda)
        .expect("Existing Owner Member must remain");

    assert_eq!(organization_after.lamports, organization_before.lamports);
    assert_eq!(organization_after.data, organization_before.data);

    assert_eq!(owner_member_after.lamports, owner_member_before.lamports);
    assert_eq!(owner_member_after.data, owner_member_before.data);
}

#[test]
fn test_initialize_organization_rolls_back_when_member_creation_fails() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator: &User = &users["creator"];
    let blocking_user: &User = &users["alice"];

    let organization_id: u64 = 2;
    let owner_member_id: u64 = 1;

    let (organization_pda, _) =
        find_organization_pda(&program_id, &creator.pubkey(), organization_id);

    let (expected_owner_member_pda, _) =
        find_member_pda(&program_id, &organization_pda, owner_member_id);

    assert!(svm.get_account(&organization_pda).is_none());
    assert!(svm.get_account(&expected_owner_member_pda).is_none());

    /*
     * Instead of supplying the expected Member PDA, deliberately supply
     * Alice's already-existing funded system account.
     *
     * Organization initialization is processed first. Owner Member
     * initialization then fails because this is not the expected Member PDA
     * and it is already an initialized account.
     */
    let invalid_owner_member = blocking_user.pubkey();

    let blocking_account_before = svm
        .get_account(&invalid_owner_member)
        .expect("Blocking account should exist");

    let ix = initialize_organization_ix_with_accounts(
        &program_id,
        &creator.pubkey(),
        &organization_pda,
        &invalid_owner_member,
        organization_id,
        owner_member_id,
    );

    let result = execute_transaction(&mut svm, &creator.pubkey(), &[creator.signer()], &[ix]);

    assert!(
        result.is_err(),
        "Initialization must fail with an invalid Owner Member account"
    );

    assert!(
        svm.get_account(&organization_pda).is_none(),
        "Organization creation must be rolled back"
    );

    assert!(
        svm.get_account(&expected_owner_member_pda).is_none(),
        "Expected Owner Member must not exist"
    );

    let blocking_account_after = svm
        .get_account(&invalid_owner_member)
        .expect("Blocking account must remain");

    assert_eq!(
        blocking_account_after.lamports,
        blocking_account_before.lamports
    );
    assert_eq!(blocking_account_after.data, blocking_account_before.data);
}
