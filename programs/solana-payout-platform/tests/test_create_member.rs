mod common;

use litesvm::LiteSVM;
use solana_payout_platform::{
    self, INITIAL_AUTHORIZATION_REVISION, ROLE_APPROVER, ROLE_EXECUTOR, ROLE_OWNER, ROLE_PREPARER,
};
use solana_pubkey::Pubkey;

use crate::common::{
    accounts::{member, member_wallet},
    executor::{create_member, initialize_organization},
    fixtures::setup_test_context,
    pda::{find_member_pda, find_member_wallet_pda, find_organization_pda},
    users::User,
};

fn initialize_test_organization(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    creator: &User,
) -> (Pubkey, Pubkey) {
    let organization_id: u64 = 1;
    let owner_member_id: u64 = 1;

    initialize_organization(program_id, svm, creator, organization_id, owner_member_id).unwrap();

    let (organization, _) = find_organization_pda(program_id, &creator.pubkey(), organization_id);

    let (owner_member, _) = find_member_pda(program_id, &organization, owner_member_id);

    (organization, owner_member)
}

#[test]
fn test_create_member_success() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let member_id: u64 = 2;
    let authorized_wallet = alice.pubkey();
    let roles = ROLE_APPROVER | ROLE_EXECUTOR;

    create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        member_id,
        &authorized_wallet,
        roles,
    )
    .unwrap();

    let (member_pda, member_bump) = find_member_pda(&program_id, &organization, member_id);

    let (member_wallet_pda, member_wallet_bump) =
        find_member_wallet_pda(&program_id, &organization, &authorized_wallet);

    let member_account = member(&svm, &member_pda);
    let wallet_index = member_wallet(&svm, &member_wallet_pda);

    assert_eq!(member_account.organization, organization);
    assert_eq!(member_account.member_id, member_id);
    assert_eq!(member_account.authorized_wallet, authorized_wallet);
    assert_eq!(
        member_account.authorization_revision,
        INITIAL_AUTHORIZATION_REVISION
    );
    assert_eq!(member_account.roles, roles);
    assert!(member_account.active);
    assert_eq!(member_account.bump, member_bump);

    assert_eq!(wallet_index.organization, organization);
    assert_eq!(wallet_index.member, member_pda);
    assert_eq!(wallet_index.authorized_wallet, authorized_wallet);
    assert_eq!(wallet_index.bump, member_wallet_bump);
}

#[test]
fn test_create_member_rejects_unauthorized_signer() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let attacker = &users["alice"];
    let bob = &users["bob"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let member_id: u64 = 2;
    let bob_wallet = bob.pubkey();

    let result = create_member(
        &program_id,
        &mut svm,
        attacker,
        &organization,
        &owner_member,
        member_id,
        &bob_wallet,
        ROLE_APPROVER,
    );

    assert!(result.is_err());

    let (member_pda, _) = find_member_pda(&program_id, &organization, member_id);

    let (wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &bob_wallet);

    assert!(svm.get_account(&member_pda).is_none());
    assert!(svm.get_account(&wallet_index).is_none());
}

#[test]
fn test_create_member_rejects_non_admin_member() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let alice_member_id: u64 = 2;
    let alice_wallet = alice.pubkey();

    create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        alice_member_id,
        &alice_wallet,
        ROLE_PREPARER,
    )
    .unwrap();

    let (alice_member, _) = find_member_pda(&program_id, &organization, alice_member_id);

    let bob_member_id: u64 = 3;
    let bob_wallet = bob.pubkey();

    let result = create_member(
        &program_id,
        &mut svm,
        alice,
        &organization,
        &alice_member,
        bob_member_id,
        &bob_wallet,
        ROLE_APPROVER,
    );

    assert!(result.is_err());

    let (bob_member, _) = find_member_pda(&program_id, &organization, bob_member_id);

    let (bob_wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &bob_wallet);

    assert!(svm.get_account(&bob_member).is_none());
    assert!(svm.get_account(&bob_wallet_index).is_none());
}

#[test]
fn test_create_member_rejects_duplicate_member_id() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let member_id: u64 = 2;
    let alice_wallet = alice.pubkey();

    create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        member_id,
        &alice_wallet,
        ROLE_APPROVER,
    )
    .unwrap();

    svm.expire_blockhash();

    let bob_wallet = bob.pubkey();

    let duplicate_result = create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        member_id,
        &bob_wallet,
        ROLE_EXECUTOR,
    );

    assert!(duplicate_result.is_err());

    let (existing_member, _) = find_member_pda(&program_id, &organization, member_id);

    let existing_member_account = member(&svm, &existing_member);

    assert_eq!(existing_member_account.authorized_wallet, alice_wallet);

    let (bob_wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &bob_wallet);

    assert!(svm.get_account(&bob_wallet_index).is_none());
}

#[test]
fn test_create_member_rejects_duplicate_wallet_and_rolls_back_member() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let alice_wallet = alice.pubkey();

    let first_member_id: u64 = 2;

    create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        first_member_id,
        &alice_wallet,
        ROLE_APPROVER,
    )
    .unwrap();

    let (first_member, _) = find_member_pda(&program_id, &organization, first_member_id);

    let (wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &alice_wallet);

    let wallet_index_before = svm.get_account(&wallet_index).unwrap();

    svm.expire_blockhash();

    let second_member_id: u64 = 3;

    let duplicate_result = create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        second_member_id,
        &alice_wallet,
        ROLE_EXECUTOR,
    );

    assert!(duplicate_result.is_err());

    let (second_member, _) = find_member_pda(&program_id, &organization, second_member_id);

    assert!(svm.get_account(&second_member).is_none());

    let wallet_index_after = svm.get_account(&wallet_index).unwrap();

    assert_eq!(wallet_index_after.lamports, wallet_index_before.lamports);
    assert_eq!(wallet_index_after.data, wallet_index_before.data);

    let wallet_index_data = member_wallet(&svm, &wallet_index);

    assert_eq!(wallet_index_data.member, first_member);
}

#[test]
fn test_create_member_rejects_invalid_roles() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let invalid_cases = [
        (2_u64, alice.pubkey(), 0_u16),
        (3_u64, bob.pubkey(), ROLE_OWNER),
    ];

    for (member_id, wallet, roles) in invalid_cases {
        svm.expire_blockhash();

        let result = create_member(
            &program_id,
            &mut svm,
            creator,
            &organization,
            &owner_member,
            member_id,
            &wallet,
            roles,
        );

        assert!(result.is_err(), "Roles {roles} should be rejected");

        let (member_pda, _) = find_member_pda(&program_id, &organization, member_id);

        let (wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &wallet);

        assert!(svm.get_account(&member_pda).is_none());
        assert!(svm.get_account(&wallet_index).is_none());
    }
}

#[test]
fn test_create_member_rejects_default_wallet() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];

    let (organization, owner_member) = initialize_test_organization(&program_id, &mut svm, creator);

    let member_id: u64 = 2;
    let invalid_wallet = Pubkey::default();

    let result = create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        member_id,
        &invalid_wallet,
        ROLE_APPROVER,
    );

    assert!(result.is_err());

    let (member_pda, _) = find_member_pda(&program_id, &organization, member_id);

    let (wallet_index, _) = find_member_wallet_pda(&program_id, &organization, &invalid_wallet);

    assert!(svm.get_account(&member_pda).is_none());
    assert!(svm.get_account(&wallet_index).is_none());
}
