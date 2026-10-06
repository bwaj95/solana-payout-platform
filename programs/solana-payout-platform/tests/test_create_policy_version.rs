mod common;

use solana_payout_platform::{self, ROLE_APPROVER, ROLE_EXECUTOR};

use crate::common::users::User;
use crate::common::{
    accounts::approval_policy_version,
    executor::{create_member, create_policy_version, initialize_organization},
    fixtures::setup_test_context,
    pda::{find_member_pda, find_organization_pda, find_policy_version_pda},
};
use litesvm::LiteSVM;
use solana_pubkey::Pubkey;

struct PolicySetup {
    organization: Pubkey,
    owner_member: Pubkey,
    alice_member: Pubkey,
    bob_member: Pubkey,
    carol_member: Pubkey,
}

fn setup_policy_members(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    creator: &User,
    alice: &User,
    bob: &User,
    carol: &User,
) -> PolicySetup {
    let organization_id: u64 = 1;
    let owner_member_id: u64 = 1;

    initialize_organization(program_id, svm, creator, organization_id, owner_member_id).unwrap();

    let (organization, _) = find_organization_pda(program_id, &creator.pubkey(), organization_id);

    let (owner_member, _) = find_member_pda(program_id, &organization, owner_member_id);

    create_member(
        program_id,
        svm,
        creator,
        &organization,
        &owner_member,
        2,
        &alice.pubkey(),
        ROLE_APPROVER,
    )
    .unwrap();

    let (alice_member, _) = find_member_pda(program_id, &organization, 2);

    create_member(
        program_id,
        svm,
        creator,
        &organization,
        &owner_member,
        3,
        &bob.pubkey(),
        ROLE_APPROVER | ROLE_EXECUTOR,
    )
    .unwrap();

    let (bob_member, _) = find_member_pda(program_id, &organization, 3);

    // Carol is deliberately not an approver.
    create_member(
        program_id,
        svm,
        creator,
        &organization,
        &owner_member,
        4,
        &carol.pubkey(),
        ROLE_EXECUTOR,
    )
    .unwrap();

    let (carol_member, _) = find_member_pda(program_id, &organization, 4);

    PolicySetup {
        organization,
        owner_member,
        alice_member,
        bob_member,
        carol_member,
    }
}

#[test]
fn test_create_policy_version_success() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];

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

    let (organization, _) = find_organization_pda(&program_id, &creator.pubkey(), organization_id);

    let (owner_member, _) = find_member_pda(&program_id, &organization, owner_member_id);

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
        ROLE_APPROVER,
    )
    .unwrap();

    let (alice_member, _) = find_member_pda(&program_id, &organization, alice_member_id);

    let bob_member_id: u64 = 3;
    let bob_wallet = bob.pubkey();

    /*
     * Bob deliberately has two roles. This proves the policy handler
     * checks for the APPROVER bit rather than demanding exact equality.
     */
    create_member(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        bob_member_id,
        &bob_wallet,
        ROLE_APPROVER | ROLE_EXECUTOR,
    )
    .unwrap();

    let (bob_member, _) = find_member_pda(&program_id, &organization, bob_member_id);

    let eligible_members = vec![alice_member, bob_member];

    let policy_id: u64 = 1;
    let version: u64 = 1;
    let threshold: u8 = 2;

    create_policy_version(
        &program_id,
        &mut svm,
        creator,
        &organization,
        &owner_member,
        policy_id,
        version,
        threshold,
        &eligible_members,
    )
    .unwrap();

    let (policy_version_pda, policy_bump) =
        find_policy_version_pda(&program_id, &organization, policy_id, version);

    let policy = approval_policy_version(&svm, &policy_version_pda);

    assert_eq!(policy.organization, organization);
    assert_eq!(policy.policy_id, policy_id);
    assert_eq!(policy.version, version);
    assert_eq!(policy.threshold, threshold);
    assert_eq!(policy.eligible_members, eligible_members);
    assert!(policy.enabled);
    assert_eq!(policy.created_by_member, owner_member);
    assert_eq!(policy.bump, policy_bump);
}

#[test]
fn test_create_policy_version_rejects_unauthorized_signer() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];
    let carol = &users["carol"];

    let setup = setup_policy_members(&program_id, &mut svm, creator, alice, bob, carol);

    let policy_id: u64 = 2;
    let version: u64 = 1;

    let eligible_members = vec![setup.alice_member, setup.bob_member];

    /*
     * Alice signs but the supplied admin_member belongs to the creator.
     * admin_member.authorized_wallet != Alice.
     */
    let result = create_policy_version(
        &program_id,
        &mut svm,
        alice,
        &setup.organization,
        &setup.owner_member,
        policy_id,
        version,
        2,
        &eligible_members,
    );

    assert!(result.is_err());

    let (policy, _) = find_policy_version_pda(&program_id, &setup.organization, policy_id, version);

    assert!(svm.get_account(&policy).is_none());
}

#[test]
fn test_create_policy_version_rejects_invalid_thresholds() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];
    let carol = &users["carol"];

    let setup = setup_policy_members(&program_id, &mut svm, creator, alice, bob, carol);

    let eligible_members = vec![setup.alice_member, setup.bob_member];

    // Two eligible Members means threshold 0 and threshold 3 are invalid.
    let invalid_cases = [(10_u64, 0_u8), (11_u64, 3_u8)];

    for (policy_id, threshold) in invalid_cases {
        let version: u64 = 1;

        let result = create_policy_version(
            &program_id,
            &mut svm,
            creator,
            &setup.organization,
            &setup.owner_member,
            policy_id,
            version,
            threshold,
            &eligible_members,
        );

        assert!(result.is_err(), "Threshold {threshold} should be rejected");

        let (policy, _) =
            find_policy_version_pda(&program_id, &setup.organization, policy_id, version);

        assert!(svm.get_account(&policy).is_none());
    }
}

#[test]
fn test_create_policy_version_rejects_duplicate_member() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];
    let carol = &users["carol"];

    let setup = setup_policy_members(&program_id, &mut svm, creator, alice, bob, carol);

    let policy_id: u64 = 20;
    let version: u64 = 1;

    let duplicated_members = vec![setup.alice_member, setup.alice_member];

    let result = create_policy_version(
        &program_id,
        &mut svm,
        creator,
        &setup.organization,
        &setup.owner_member,
        policy_id,
        version,
        2,
        &duplicated_members,
    );

    assert!(result.is_err());

    let (policy, _) = find_policy_version_pda(&program_id, &setup.organization, policy_id, version);

    assert!(svm.get_account(&policy).is_none());
}

#[test]
fn test_create_policy_version_rejects_cross_organization_member() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];
    let carol = &users["carol"];
    let dan = &users["dan"];

    let setup = setup_policy_members(&program_id, &mut svm, creator, alice, bob, carol);

    // Create a second Organization under the same creator.
    let second_organization_id: u64 = 2;
    let second_owner_member_id: u64 = 1;

    initialize_organization(
        &program_id,
        &mut svm,
        creator,
        second_organization_id,
        second_owner_member_id,
    )
    .unwrap();

    let (second_organization, _) =
        find_organization_pda(&program_id, &creator.pubkey(), second_organization_id);

    let (second_owner_member, _) =
        find_member_pda(&program_id, &second_organization, second_owner_member_id);

    let dan_member_id: u64 = 2;

    create_member(
        &program_id,
        &mut svm,
        creator,
        &second_organization,
        &second_owner_member,
        dan_member_id,
        &dan.pubkey(),
        ROLE_APPROVER,
    )
    .unwrap();

    let (dan_member, _) = find_member_pda(&program_id, &second_organization, dan_member_id);

    let policy_id: u64 = 40;
    let version: u64 = 1;

    let mixed_organization_members = vec![setup.alice_member, dan_member];

    let result = create_policy_version(
        &program_id,
        &mut svm,
        creator,
        &setup.organization,
        &setup.owner_member,
        policy_id,
        version,
        2,
        &mixed_organization_members,
    );

    assert!(result.is_err());

    let (policy, _) = find_policy_version_pda(&program_id, &setup.organization, policy_id, version);

    assert!(svm.get_account(&policy).is_none());
}

#[test]
fn test_create_policy_version_is_immutable() {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let creator = &users["creator"];
    let alice = &users["alice"];
    let bob = &users["bob"];
    let carol = &users["carol"];

    let setup = setup_policy_members(&program_id, &mut svm, creator, alice, bob, carol);

    let policy_id: u64 = 50;
    let version: u64 = 1;

    let original_members = vec![setup.alice_member, setup.bob_member];

    create_policy_version(
        &program_id,
        &mut svm,
        creator,
        &setup.organization,
        &setup.owner_member,
        policy_id,
        version,
        2,
        &original_members,
    )
    .unwrap();

    let (policy, _) = find_policy_version_pda(&program_id, &setup.organization, policy_id, version);

    let policy_before = svm.get_account(&policy).unwrap();

    svm.expire_blockhash();

    /*
     * Attempt to recreate the same PDA with weaker terms.
     * Anchor init must reject it before existing data can change.
     */
    let replacement_attempt = create_policy_version(
        &program_id,
        &mut svm,
        creator,
        &setup.organization,
        &setup.owner_member,
        policy_id,
        version,
        1,
        &[setup.alice_member],
    );

    assert!(replacement_attempt.is_err());

    let policy_after = svm.get_account(&policy).unwrap();

    assert_eq!(policy_after.lamports, policy_before.lamports);
    assert_eq!(policy_after.data, policy_before.data);

    let stored_policy = approval_policy_version(&svm, &policy);

    assert_eq!(stored_policy.threshold, 2);
    assert_eq!(stored_policy.eligible_members, original_members);
}
