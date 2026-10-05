mod common;

use solana_payout_platform::{self, INITIAL_AUTHORIZATION_REVISION, INITIAL_OWNER_ROLES};

use crate::common::{
    accounts::{member, member_wallet, organization},
    executor::{execute_transaction, initialize_organization},
    fixtures::setup_test_context,
    instructions::initialize_organization_ix_with_accounts,
    pda::{find_member_pda, find_member_wallet_pda, find_organization_pda},
    users::User,
};

#[test]
fn test_initialize_organization_success() {
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

    let (organization_pda, organization_bump) =
        find_organization_pda(&program_id, &creator.pubkey(), organization_id);

    let (owner_member_pda, owner_member_bump) =
        find_member_pda(&program_id, &organization_pda, owner_member_id);

    let (owner_member_wallet_pda, owner_member_wallet_bump) =
        find_member_wallet_pda(&program_id, &organization_pda, &creator.pubkey());

    let organization_account = organization(&svm, &organization_pda);

    let owner_member_account = member(&svm, &owner_member_pda);

    let owner_member_wallet_account = member_wallet(&svm, &owner_member_wallet_pda);

    assert_eq!(organization_account.organization_id, organization_id);
    assert_eq!(organization_account.creator, creator.pubkey());
    assert_eq!(organization_account.owner_member, owner_member_pda);
    assert!(!organization_account.paused);
    assert_eq!(organization_account.bump, organization_bump);

    assert_eq!(owner_member_account.organization, organization_pda);
    assert_eq!(owner_member_account.member_id, owner_member_id);
    assert_eq!(owner_member_account.authorized_wallet, creator.pubkey());
    assert_eq!(
        owner_member_account.authorization_revision,
        INITIAL_AUTHORIZATION_REVISION
    );
    assert_eq!(owner_member_account.roles, INITIAL_OWNER_ROLES);
    assert!(owner_member_account.active);
    assert_eq!(owner_member_account.bump, owner_member_bump);

    assert_eq!(owner_member_wallet_account.organization, organization_pda);
    assert_eq!(owner_member_wallet_account.member, owner_member_pda);
    assert_eq!(
        owner_member_wallet_account.authorized_wallet,
        creator.pubkey()
    );
    assert_eq!(owner_member_wallet_account.bump, owner_member_wallet_bump);
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

    let (owner_member_wallet_pda, _) =
        find_member_wallet_pda(&program_id, &organization_pda, &creator.pubkey());

    let organization_before = svm.get_account(&organization_pda).unwrap();

    let owner_member_before = svm.get_account(&owner_member_pda).unwrap();

    let owner_member_wallet_before = svm.get_account(&owner_member_wallet_pda).unwrap();

    svm.expire_blockhash();

    let result = initialize_organization(
        &program_id,
        &mut svm,
        creator,
        organization_id,
        owner_member_id,
    );

    assert!(result.is_err());

    let organization_after = svm.get_account(&organization_pda).unwrap();

    let owner_member_after = svm.get_account(&owner_member_pda).unwrap();

    let owner_member_wallet_after = svm.get_account(&owner_member_wallet_pda).unwrap();

    assert_eq!(organization_after.lamports, organization_before.lamports);
    assert_eq!(organization_after.data, organization_before.data);

    assert_eq!(owner_member_after.lamports, owner_member_before.lamports);
    assert_eq!(owner_member_after.data, owner_member_before.data);

    assert_eq!(
        owner_member_wallet_after.lamports,
        owner_member_wallet_before.lamports
    );
    assert_eq!(
        owner_member_wallet_after.data,
        owner_member_wallet_before.data
    );
}

#[test]
fn test_initialize_organization_rolls_back_if_wallet_index_fails() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator: &User = &users["creator"];
    let blocking_user: &User = &users["alice"];

    let organization_id: u64 = 2;
    let owner_member_id: u64 = 1;

    let (organization_pda, _) =
        find_organization_pda(&program_id, &creator.pubkey(), organization_id);

    let (owner_member_pda, _) = find_member_pda(&program_id, &organization_pda, owner_member_id);

    let (expected_wallet_index, _) =
        find_member_wallet_pda(&program_id, &organization_pda, &creator.pubkey());

    /*
     * Supply Alice's existing account where the expected MemberWallet PDA
     * should be. Organization and Owner Member initialization occur first,
     * and wallet-index validation then fails. The complete transaction
     * must roll back.
     */
    let invalid_wallet_index = blocking_user.pubkey();

    let blocking_account_before = svm.get_account(&invalid_wallet_index).unwrap();

    let ix = initialize_organization_ix_with_accounts(
        &program_id,
        &creator.pubkey(),
        &organization_pda,
        &owner_member_pda,
        &invalid_wallet_index,
        organization_id,
        owner_member_id,
    );

    let result = execute_transaction(&mut svm, &creator.pubkey(), &[creator.signer()], &[ix]);

    assert!(result.is_err());

    assert!(svm.get_account(&organization_pda).is_none());
    assert!(svm.get_account(&owner_member_pda).is_none());
    assert!(svm.get_account(&expected_wallet_index).is_none());

    let blocking_account_after = svm.get_account(&invalid_wallet_index).unwrap();

    assert_eq!(
        blocking_account_after.lamports,
        blocking_account_before.lamports
    );
    assert_eq!(blocking_account_after.data, blocking_account_before.data);
}
