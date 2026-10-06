mod common;

use std::collections::HashMap;

use anchor_lang::{solana_program::program_pack::Pack, AccountDeserialize, AccountSerialize};
use anchor_spl::token::spl_token::state::{Account as SplTokenAccount, Mint as SplMint};
use litesvm::LiteSVM;

use solana_payout_platform::{self, Asset, Member, ROLE_APPROVER};
use solana_pubkey::Pubkey;

use crate::common::{
    accounts::get_vault_state,
    executor::{initialize_organization, initialize_vault},
    fixtures::setup_test_context,
    pda::{find_member_pda, find_organization_pda, find_vault_state_pda, find_vault_token_account},
    users::User,
};

fn add_test_mint(svm: &mut LiteSVM) -> Pubkey {
    let mint = Pubkey::new_unique();

    let mint_state = SplMint {
        mint_authority: Default::default(),
        supply: 0,
        decimals: 6,
        is_initialized: true,
        freeze_authority: Default::default(),
    };

    let mut mint_data = vec![0; SplMint::LEN];
    SplMint::pack(mint_state, &mut mint_data).unwrap();

    // Reuse an existing LiteSVM account value as a convenient Account
    // container, then replace all fields relevant to the test mint.
    let mut mint_account = svm
        .get_account(&anchor_spl::token::ID)
        .expect("SPL Token Program not installed in LiteSVM");

    mint_account.lamports = svm.minimum_balance_for_rent_exemption(SplMint::LEN);
    mint_account.data = mint_data;
    mint_account.owner = anchor_spl::token::ID;
    mint_account.executable = false;
    mint_account.rent_epoch = 0;

    svm.set_account(mint, mint_account).unwrap();

    mint
}

fn update_member<F>(svm: &mut LiteSVM, member_pda: &Pubkey, update: F)
where
    F: FnOnce(&mut Member),
{
    let mut account = svm
        .get_account(member_pda)
        .expect("Member account not found");

    let mut data: &[u8] = &account.data;
    let mut member = Member::try_deserialize(&mut data).unwrap();

    update(&mut member);

    let mut serialized = Vec::new();
    member.try_serialize(&mut serialized).unwrap();

    assert!(serialized.len() <= account.data.len());
    serialized.resize(account.data.len(), 0);

    account.data = serialized;

    svm.set_account(*member_pda, account).unwrap();
}

fn setup_organization() -> (LiteSVM, HashMap<String, User>, Pubkey, Pubkey) {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let organization_id = 1;
    let creator = &users["creator"];
    let owner_member_id = creator.id();

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

    (svm, users, organization, owner_member)
}

#[test]
fn test_initialize_vault_success() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 1;

    initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    )
    .unwrap();

    let (vault_state_pda, vault_bump) = find_vault_state_pda(&program_id, &organization, vault_id);

    let vault_token_account = find_vault_token_account(&vault_state_pda, &mint);

    let vault_state = get_vault_state(&svm, &vault_state_pda);

    assert_eq!(vault_state.organization, organization);
    assert_eq!(vault_state.vault_id, vault_id);
    assert_eq!(vault_state.reserved_total, 0);
    assert!(vault_state.active);
    assert_eq!(vault_state.bump, vault_bump);

    match vault_state.asset {
        Asset::Spl { mint: stored_mint } => {
            assert_eq!(stored_mint, mint);
        }
        Asset::NativeSol => {
            panic!("Expected an SPL vault");
        }
    }

    let raw_token_account = svm
        .get_account(&vault_token_account)
        .expect("Vault token account not found");

    assert_eq!(raw_token_account.owner, anchor_spl::token::ID);

    let token_account = SplTokenAccount::unpack(&raw_token_account.data).unwrap();

    assert_eq!(token_account.mint, mint);
    assert_eq!(token_account.owner, vault_state_pda);
    assert_eq!(token_account.amount, 0);
}

#[test]
fn test_initialize_vault_rejects_zero_vault_id() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 0;

    let result = initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    );

    assert!(result.is_err());

    let (vault_state, _) = find_vault_state_pda(&program_id, &organization, vault_id);

    let vault_token_account = find_vault_token_account(&vault_state, &mint);

    // The handler failed, so account creation must also roll back.
    assert!(svm.get_account(&vault_state).is_none());
    assert!(svm.get_account(&vault_token_account).is_none());
}

#[test]
fn test_initialize_vault_rejects_unauthorized_wallet() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    let alice = &users["alice"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 1;

    let result = initialize_vault(
        &mut svm,
        alice,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    );

    assert!(result.is_err());

    let (vault_state, _) = find_vault_state_pda(&program_id, &organization, vault_id);

    assert!(svm.get_account(&vault_state).is_none());
}

#[test]
fn test_initialize_vault_rejects_inactive_admin() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    update_member(&mut svm, &owner_member, |member| {
        member.active = false;
    });

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 1;

    let result = initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    );

    assert!(result.is_err());

    let (vault_state, _) = find_vault_state_pda(&program_id, &organization, vault_id);

    assert!(svm.get_account(&vault_state).is_none());
}

#[test]
fn test_initialize_vault_rejects_member_without_admin_role() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    update_member(&mut svm, &owner_member, |member| {
        member.roles = ROLE_APPROVER;
    });

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 1;

    let result = initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    );

    assert!(result.is_err());

    let (vault_state, _) = find_vault_state_pda(&program_id, &organization, vault_id);

    assert!(svm.get_account(&vault_state).is_none());
}

#[test]
fn test_initialize_vault_rejects_duplicate_vault_id() {
    let (mut svm, users, organization, owner_member) = setup_organization();

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);
    let vault_id = 1;

    initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    )
    .unwrap();

    // Ensure the second transaction has a different recent blockhash.
    svm.expire_blockhash();

    let duplicate_result = initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    );

    assert!(duplicate_result.is_err());
}

#[test]
fn test_same_mint_can_have_multiple_vaults() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member) = setup_organization();

    let creator = &users["creator"];
    let mint = add_test_mint(&mut svm);

    initialize_vault(&mut svm, creator, &organization, &owner_member, &mint, 1).unwrap();

    initialize_vault(&mut svm, creator, &organization, &owner_member, &mint, 2).unwrap();

    let (vault_one, _) = find_vault_state_pda(&program_id, &organization, 1);

    let (vault_two, _) = find_vault_state_pda(&program_id, &organization, 2);

    let vault_one_ata = find_vault_token_account(&vault_one, &mint);

    let vault_two_ata = find_vault_token_account(&vault_two, &mint);

    assert_ne!(vault_one, vault_two);
    assert_ne!(vault_one_ata, vault_two_ata);

    assert!(svm.get_account(&vault_one).is_some());
    assert!(svm.get_account(&vault_two).is_some());
    assert!(svm.get_account(&vault_one_ata).is_some());
    assert!(svm.get_account(&vault_two_ata).is_some());

    match get_vault_state(&svm, &vault_one).asset {
        Asset::Spl { mint: stored_mint } => {
            assert_eq!(stored_mint, mint);
        }
        Asset::NativeSol => panic!("Expected an SPL vault"),
    }

    match get_vault_state(&svm, &vault_two).asset {
        Asset::Spl { mint: stored_mint } => {
            assert_eq!(stored_mint, mint);
        }
        Asset::NativeSol => panic!("Expected an SPL vault"),
    }
}
