mod common;

use std::collections::HashMap;

use anchor_lang::{
    solana_program::program_pack::Pack, AccountDeserialize, AccountSerialize, InstructionData,
    ToAccountMetas,
};
use anchor_spl::token::spl_token::state::{
    Account as SplTokenAccount, AccountState, Mint as SplMint,
};
use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_message::Instruction;
use solana_payout_platform::{
    self,
    accounts::{InitializeVault, RegisterRecipient},
    instruction, Member, Organization, Recipient, VaultState, INITIAL_RECIPIENT_WALLET_REVISION,
    RECIPIENT_SEED, ROLE_APPROVER, VAULT_SEED,
};
use solana_pubkey::Pubkey;

use crate::common::{
    executor::{execute_transaction, initialize_organization},
    fixtures::setup_test_context,
    pda::{find_member_pda, find_organization_pda},
    users::User,
};

fn find_vault_state_pda(program_id: &Pubkey, organization: &Pubkey, vault_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            VAULT_SEED,
            organization.as_ref(),
            vault_id.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

fn find_recipient_pda(
    program_id: &Pubkey,
    organization: &Pubkey,
    recipient_id: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            RECIPIENT_SEED,
            organization.as_ref(),
            recipient_id.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

fn find_associated_token_account(authority: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            authority.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
    .0
}

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

    let mut mint_account = svm
        .get_account(&anchor_spl::token::ID)
        .expect("SPL Token Program not installed");

    mint_account.lamports = svm.minimum_balance_for_rent_exemption(SplMint::LEN);
    mint_account.data = mint_data;
    mint_account.owner = anchor_spl::token::ID;
    mint_account.executable = false;
    mint_account.rent_epoch = 0;

    svm.set_account(mint, mint_account).unwrap();

    mint
}

fn add_test_token_account(
    svm: &mut LiteSVM,
    token_account_address: Pubkey,
    mint: Pubkey,
    authority: Pubkey,
) {
    let token_account_state = SplTokenAccount {
        mint,
        owner: authority,
        amount: 0,
        delegate: Default::default(),
        state: AccountState::Initialized,
        is_native: Default::default(),
        delegated_amount: 0,
        close_authority: Default::default(),
    };

    let mut token_account_data = vec![0; SplTokenAccount::LEN];

    SplTokenAccount::pack(token_account_state, &mut token_account_data).unwrap();

    let mut account = svm
        .get_account(&anchor_spl::token::ID)
        .expect("SPL Token Program not installed");

    account.lamports = svm.minimum_balance_for_rent_exemption(SplTokenAccount::LEN);
    account.data = token_account_data;
    account.owner = anchor_spl::token::ID;
    account.executable = false;
    account.rent_epoch = 0;

    svm.set_account(token_account_address, account).unwrap();
}

fn initialize_vault_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    admin_member: &Pubkey,
    mint: &Pubkey,
    vault_id: u64,
) -> Instruction {
    let (vault_state, _) = find_vault_state_pda(program_id, organization, vault_id);

    let vault_token_account = find_associated_token_account(&vault_state, mint);

    let accounts = InitializeVault {
        authority: *authority,
        organization: *organization,
        admin_member: *admin_member,
        vault_state,
        mint: *mint,
        vault_token_account,
        token_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: anchor_lang::system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::InitializeVault { vault_id }.data(),
    }
}

fn initialize_vault(
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    admin_member: &Pubkey,
    mint: &Pubkey,
    vault_id: u64,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let program_id = solana_payout_platform::ID;

    let ix = initialize_vault_ix(
        &program_id,
        &authority.pubkey(),
        organization,
        admin_member,
        mint,
        vault_id,
    );

    let payer = authority.pubkey();
    let signer = authority.signer();

    execute_transaction(svm, &payer, &[signer], &[ix])
}

fn register_recipient_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    registrar_member: &Pubkey,
    vault_state: &Pubkey,
    mint: &Pubkey,
    destination_token_account: &Pubkey,
    recipient_id: u64,
    destination_wallet: Pubkey,
) -> Instruction {
    let (recipient, _) = find_recipient_pda(program_id, organization, recipient_id);

    let accounts = RegisterRecipient {
        authority: *authority,
        organization: *organization,
        registrar_member: *registrar_member,
        vault_state: *vault_state,
        recipient,
        mint: *mint,
        destination_token_account: *destination_token_account,
        system_program: anchor_lang::system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::RegisterRecipient {
            recipient_id,
            destination_wallet,
        }
        .data(),
    }
}

fn register_recipient(
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    registrar_member: &Pubkey,
    vault_state: &Pubkey,
    mint: &Pubkey,
    destination_token_account: &Pubkey,
    recipient_id: u64,
    destination_wallet: Pubkey,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let program_id = solana_payout_platform::ID;

    let ix = register_recipient_ix(
        &program_id,
        &authority.pubkey(),
        organization,
        registrar_member,
        vault_state,
        mint,
        destination_token_account,
        recipient_id,
        destination_wallet,
    );

    let payer = authority.pubkey();
    let signer = authority.signer();

    execute_transaction(svm, &payer, &[signer], &[ix])
}

fn get_recipient(svm: &LiteSVM, recipient: &Pubkey) -> Recipient {
    let account = svm
        .get_account(recipient)
        .expect("Recipient account not found");

    let mut data: &[u8] = &account.data;

    Recipient::try_deserialize(&mut data).unwrap()
}

fn update_anchor_account<T, F>(svm: &mut LiteSVM, address: &Pubkey, update: F)
where
    T: AccountDeserialize + AccountSerialize,
    F: FnOnce(&mut T),
{
    let mut account = svm.get_account(address).expect("Anchor account not found");

    let mut data: &[u8] = &account.data;
    let mut value = T::try_deserialize(&mut data).unwrap();

    update(&mut value);

    let mut serialized = Vec::new();
    value.try_serialize(&mut serialized).unwrap();

    assert!(serialized.len() <= account.data.len());
    serialized.resize(account.data.len(), 0);

    account.data = serialized;

    svm.set_account(*address, account).unwrap();
}

fn setup_vault() -> (
    LiteSVM,
    HashMap<String, User>,
    Pubkey,
    Pubkey,
    Pubkey,
    Pubkey,
) {
    let program_id = solana_payout_platform::ID;
    let (mut svm, users) = setup_test_context(&program_id);

    let organization_id = 1;
    let vault_id = 1;

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

    let mint = add_test_mint(&mut svm);

    initialize_vault(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &mint,
        vault_id,
    )
    .unwrap();

    let (vault_state, _) = find_vault_state_pda(&program_id, &organization, vault_id);

    (svm, users, organization, owner_member, vault_state, mint)
}

#[test]
fn test_register_recipient_success() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();
    let recipient_id = 1;

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        recipient_id,
        destination_wallet,
    )
    .unwrap();

    let (recipient_pda, recipient_bump) =
        find_recipient_pda(&program_id, &organization, recipient_id);

    let recipient = get_recipient(&svm, &recipient_pda);

    assert_eq!(recipient.organization, organization);
    assert_eq!(recipient.recipient_id, recipient_id);
    assert_eq!(recipient.current_destination, destination_wallet);
    assert_eq!(recipient.wallet_revision, INITIAL_RECIPIENT_WALLET_REVISION);
    assert!(recipient.active);
    assert_eq!(recipient.bump, recipient_bump);
}

#[test]
fn test_register_recipient_rejects_invalid_identity_fields() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    let zero_id_result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        0,
        destination_wallet,
    );

    assert!(zero_id_result.is_err());

    let (zero_recipient, _) = find_recipient_pda(&program_id, &organization, 0);

    assert!(svm.get_account(&zero_recipient).is_none());

    let default_destination = Pubkey::default();

    let default_destination_ata = find_associated_token_account(&default_destination, &mint);

    add_test_token_account(&mut svm, default_destination_ata, mint, default_destination);

    let default_destination_result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &default_destination_ata,
        1,
        default_destination,
    );

    assert!(default_destination_result.is_err());

    let (recipient_one, _) = find_recipient_pda(&program_id, &organization, 1);

    assert!(svm.get_account(&recipient_one).is_none());
}

#[test]
fn test_register_recipient_rejects_unauthorized_wallet() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let alice = &users["alice"];
    let destination_wallet = alice.pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    let result = register_recipient(
        &mut svm,
        alice,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_recipient_rejects_inactive_or_unprivileged_member() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    update_anchor_account::<Member, _>(&mut svm, &owner_member, |member| {
        member.active = false;
    });

    let inactive_result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(inactive_result.is_err());

    update_anchor_account::<Member, _>(&mut svm, &owner_member, |member| {
        member.active = true;
        member.roles = ROLE_APPROVER;
    });

    let role_result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        2,
        destination_wallet,
    );

    assert!(role_result.is_err());
}

#[test]
fn test_register_recipient_rejects_paused_organization() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    update_anchor_account::<Organization, _>(&mut svm, &organization, |organization| {
        organization.paused = true;
    });

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    let result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_recipient_rejects_inactive_vault() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    update_anchor_account::<VaultState, _>(&mut svm, &vault_state, |vault| {
        vault.active = false;
    });

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    let result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_recipient_rejects_wrong_mint() {
    let (mut svm, users, organization, owner_member, vault_state, _vault_mint) = setup_vault();

    let creator = &users["creator"];
    let wrong_mint = add_test_mint(&mut svm);
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &wrong_mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        wrong_mint,
        destination_wallet,
    );

    let result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &wrong_mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_recipient_rejects_noncanonical_token_account() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    // Correct mint and authority, but deliberately not the ATA address.
    let noncanonical_token_account = Pubkey::new_unique();

    add_test_token_account(
        &mut svm,
        noncanonical_token_account,
        mint,
        destination_wallet,
    );

    let result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &noncanonical_token_account,
        1,
        destination_wallet,
    );

    assert!(result.is_err());
}

#[test]
fn test_register_recipient_rejects_duplicate_recipient_id() {
    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    )
    .unwrap();

    svm.expire_blockhash();

    let duplicate_result = register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    );

    assert!(duplicate_result.is_err());
}

#[test]
fn test_same_destination_can_be_used_by_multiple_recipients() {
    let program_id = solana_payout_platform::ID;

    let (mut svm, users, organization, owner_member, vault_state, mint) = setup_vault();

    let creator = &users["creator"];
    let destination_wallet = users["alice"].pubkey();

    let destination_token_account = find_associated_token_account(&destination_wallet, &mint);

    add_test_token_account(
        &mut svm,
        destination_token_account,
        mint,
        destination_wallet,
    );

    register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        1,
        destination_wallet,
    )
    .unwrap();

    register_recipient(
        &mut svm,
        creator,
        &organization,
        &owner_member,
        &vault_state,
        &mint,
        &destination_token_account,
        2,
        destination_wallet,
    )
    .unwrap();

    let (recipient_one, _) = find_recipient_pda(&program_id, &organization, 1);

    let (recipient_two, _) = find_recipient_pda(&program_id, &organization, 2);

    let recipient_one = get_recipient(&svm, &recipient_one);
    let recipient_two = get_recipient(&svm, &recipient_two);

    assert_eq!(recipient_one.current_destination, destination_wallet);

    assert_eq!(recipient_two.current_destination, destination_wallet);

    assert_ne!(recipient_one.recipient_id, recipient_two.recipient_id);
}
