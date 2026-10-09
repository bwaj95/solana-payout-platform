use std::collections::HashMap;

use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_pubkey::Pubkey;

use crate::common::{pda::find_payment_pda, users::User};
use anchor_lang::AccountSerialize;
use solana_account::Account;
use solana_payout_platform::{
    compute_payment_terms_hash, Approval, ApprovalPolicyVersion, Asset, Member, Organization,
    Payment, PaymentState, Recipient, ReservationState, SettlementRail, VaultState,
    INITIAL_AUTHORIZATION_REVISION, INITIAL_PAYMENT_REVISION, INITIAL_RECIPIENT_WALLET_REVISION,
    ROLE_ADMIN, ROLE_APPROVER, ROLE_EXECUTOR, ROLE_PREPARER,
};

use crate::common::pda::{
    find_approval_pda, find_member_pda, find_organization_pda, find_policy_pda, find_recipient_pda,
    find_vault_pda,
};

use crate::common::{
    accounts::{approval_policy_version, member, organization, recipient, vault_state},
    executor::create_payment,
};

use crate::common::executor::{
    approve_payment as execute_approve_payment, cancel_payment as execute_cancel_payment,
    execute_spl_payment as execute_spl_payment_transaction,
    finalize_payment_approval as execute_finalize_payment_approval,
    rotate_recipient_wallet as execute_rotate_recipient_wallet,
};

use crate::common::accounts::{approval as load_approval, payment as load_payment};

pub const HUNDRED_SOL: u64 = 100_000_000_000u64;

const TEST_ACCOUNT_LAMPORTS: u64 = 1_000_000_000;

pub fn setup_test_context(program_id: &Pubkey) -> (LiteSVM, HashMap<String, User>) {
    let mut svm = LiteSVM::new();

    let bytes = include_bytes!("../../../../target/deploy/solana_payout_platform.so");
    svm.add_program(program_id, bytes).unwrap();

    let users = init_users_hashmap(&mut svm);

    (svm, users)
}

pub fn init_users_hashmap(svm: &mut LiteSVM) -> HashMap<String, User> {
    let creator = init_funded_user(svm, "creator");
    let alice = init_funded_user(svm, "alice");
    let bob = init_funded_user(svm, "bob");
    let carol = init_funded_user(svm, "carol");
    let dan = init_funded_user(svm, "dan");

    let mut users = HashMap::<String, User>::new();

    users.insert("creator".to_string(), creator);
    users.insert("alice".to_string(), alice);
    users.insert("bob".to_string(), bob);
    users.insert("carol".to_string(), carol);
    users.insert("dan".to_string(), dan);

    users
}

pub fn init_funded_user(svm: &mut LiteSVM, name: &str) -> User {
    let user = User::new(name);
    fund_user(&user.pubkey(), HUNDRED_SOL, svm).unwrap();

    user
}

pub fn fund_user(
    pubkey: &Pubkey,
    amount: u64,
    svm: &mut LiteSVM,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    svm.airdrop(pubkey, amount)
}

pub struct CreatePaymentFixture {
    pub svm: LiteSVM,
    pub authority: User,

    pub organization: Pubkey,
    pub preparer_member: Pubkey,

    pub recipient: Pubkey,
    pub destination: Pubkey,
    pub recipient_wallet_revision: u32,

    pub vault: Pubkey,
    pub mint: Pubkey,

    pub approval_policy_version: Pubkey,

    pub payment_id: u64,
    pub amount: u64,
    pub execute_after: i64,
}

impl CreatePaymentFixture {
    pub fn expected_terms_hash(&self) -> [u8; 32] {
        compute_payment_terms_hash(
            &self.organization,
            self.payment_id,
            solana_payout_platform::INITIAL_PAYMENT_REVISION,
            &self.recipient,
            &self.destination,
            self.recipient_wallet_revision,
            &self.vault,
            &self.mint,
            self.amount,
            &self.approval_policy_version,
            SettlementRail::PublicSpl,
            self.execute_after,
        )
    }

    pub fn submit_create_payment(
        &mut self,
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        create_payment(
            &solana_payout_platform::ID,
            &mut self.svm,
            &self.authority,
            &self.organization,
            &self.preparer_member,
            &self.recipient,
            &self.vault,
            &self.approval_policy_version,
            self.payment_id,
            self.amount,
            self.execute_after,
        )
    }

    pub fn update_organization<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Organization),
    {
        let mut account = organization(&self.svm, &self.organization);
        update(&mut account);

        store_anchor_account(&mut self.svm, &self.organization, &account);
    }

    pub fn update_preparer_member<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Member),
    {
        let mut account = member(&self.svm, &self.preparer_member);
        update(&mut account);

        store_anchor_account(&mut self.svm, &self.preparer_member, &account);
    }

    pub fn update_recipient<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Recipient),
    {
        let mut account = recipient(&self.svm, &self.recipient);
        update(&mut account);

        store_anchor_account(&mut self.svm, &self.recipient, &account);
    }

    pub fn update_vault<F>(&mut self, update: F)
    where
        F: FnOnce(&mut VaultState),
    {
        let mut account = vault_state(&self.svm, &self.vault);
        update(&mut account);

        store_anchor_account(&mut self.svm, &self.vault, &account);
    }

    pub fn update_policy<F>(&mut self, update: F)
    where
        F: FnOnce(&mut ApprovalPolicyVersion),
    {
        let mut account = approval_policy_version(&self.svm, &self.approval_policy_version);

        update(&mut account);

        store_anchor_account(&mut self.svm, &self.approval_policy_version, &account);
    }
}

fn store_anchor_account<T: AccountSerialize>(svm: &mut LiteSVM, address: &Pubkey, value: &T) {
    let mut data = Vec::new();
    value.try_serialize(&mut data).unwrap();

    svm.set_account(
        *address,
        Account {
            lamports: TEST_ACCOUNT_LAMPORTS,
            data,
            owner: solana_payout_platform::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

pub fn setup_create_payment_fixture() -> CreatePaymentFixture {
    let program_id = solana_payout_platform::ID;
    let (mut svm, mut users) = setup_test_context(&program_id);

    let authority = users
        .remove("creator")
        .expect("Creator test user must exist");

    let organization_id = 1;
    let preparer_member_id = 1;
    let recipient_id = 1;
    let vault_id = 1;
    let policy_id = 1;
    let policy_version = 1;

    let payment_id = 1;
    let amount = 2_000_000;
    let execute_after = 0;

    let (organization, organization_bump) =
        find_organization_pda(&program_id, &authority.pubkey(), organization_id);

    let (preparer_member, preparer_member_bump) =
        find_member_pda(&program_id, &organization, preparer_member_id);

    let (recipient, recipient_bump) = find_recipient_pda(&program_id, &organization, recipient_id);

    let (vault, vault_bump) = find_vault_pda(&program_id, &organization, vault_id);

    let (approval_policy_version, policy_bump) =
        find_policy_pda(&program_id, &organization, policy_id, policy_version);

    let mint = Pubkey::new_unique();
    let destination = Pubkey::new_unique();

    store_anchor_account(
        &mut svm,
        &organization,
        &Organization {
            organization_id,
            creator: authority.pubkey(),
            owner_member: preparer_member,
            paused: false,
            bump: organization_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &preparer_member,
        &Member {
            organization,
            member_id: preparer_member_id,
            authorized_wallet: authority.pubkey(),
            authorization_revision: INITIAL_AUTHORIZATION_REVISION,
            roles: ROLE_PREPARER,
            active: true,
            bump: preparer_member_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &recipient,
        &Recipient {
            organization,
            recipient_id,
            current_destination: destination,
            wallet_revision: INITIAL_RECIPIENT_WALLET_REVISION,
            active: true,
            bump: recipient_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &vault,
        &VaultState {
            organization,
            vault_id,
            asset: Asset::Spl { mint },
            reserved_total: 0,
            active: true,
            bump: vault_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &approval_policy_version,
        &ApprovalPolicyVersion {
            organization,
            policy_id,
            version: policy_version,
            created_by_member: preparer_member,
            eligible_members: vec![preparer_member],
            threshold: 1,
            enabled: true,
            bump: policy_bump,
        },
    );

    CreatePaymentFixture {
        svm,
        authority,
        organization,
        preparer_member,
        recipient,
        destination,
        recipient_wallet_revision: INITIAL_RECIPIENT_WALLET_REVISION,
        vault,
        mint,
        approval_policy_version,
        payment_id,
        amount,
        execute_after,
    }
}

const TEST_USDC_DECIMALS: u8 = 6;
const TEST_VAULT_BALANCE: u64 = 10_000_000;
const TEST_PAYMENT_AMOUNT: u64 = 2_000_000;

pub struct TestApprover {
    pub user: User,
    pub member: Pubkey,
    pub member_id: u64,
    pub authorization_revision: u64,
}

pub struct ApprovalFixture {
    pub svm: LiteSVM,

    pub finalizer: User,
    pub finalizer_member: Pubkey,

    pub organization: Pubkey,
    pub approvers: Vec<TestApprover>,

    pub recipient: Pubkey,
    pub vault_state: Pubkey,
    pub mint: Pubkey,
    pub vault_ata: Pubkey,

    pub approval_policy_version: Pubkey,
    pub payment: Pubkey,

    pub payment_id: u64,
    pub payment_revision: u32,
    pub payment_amount: u64,
}

impl ApprovalFixture {
    pub fn approval_pda(&self, approver_index: usize) -> Pubkey {
        let approver = &self.approvers[approver_index];

        find_approval_pda(
            &solana_payout_platform::ID,
            &self.organization,
            self.payment_id,
            self.payment_revision,
            approver.member_id,
            approver.authorization_revision,
        )
        .0
    }

    pub fn approval_witnesses(&self, approver_indices: &[usize]) -> Vec<(Pubkey, Pubkey)> {
        approver_indices
            .iter()
            .map(|index| {
                let approver = &self.approvers[*index];

                (self.approval_pda(*index), approver.member)
            })
            .collect()
    }

    pub fn approve(
        &mut self,
        approver_index: usize,
        previous_approver_indices: &[usize],
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let witnesses = self.approval_witnesses(previous_approver_indices);

        let approval = self.approval_pda(approver_index);
        let approver = &self.approvers[approver_index];

        execute_approve_payment(
            &solana_payout_platform::ID,
            &mut self.svm,
            &approver.user,
            &self.organization,
            &approver.member,
            &self.vault_state,
            &self.mint,
            &self.vault_ata,
            &self.recipient,
            &self.approval_policy_version,
            &self.payment,
            &approval,
            self.payment_id,
            &witnesses,
        )
    }

    pub fn finalize(
        &mut self,
        approved_member_indices: &[usize],
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let witnesses = self.approval_witnesses(approved_member_indices);

        execute_finalize_payment_approval(
            &solana_payout_platform::ID,
            &mut self.svm,
            &self.finalizer,
            &self.organization,
            &self.finalizer_member,
            &self.vault_state,
            &self.mint,
            &self.vault_ata,
            &self.recipient,
            &self.approval_policy_version,
            &self.payment,
            self.payment_id,
            &witnesses,
        )
    }

    pub fn set_vault_token_balance(&mut self, amount: u64) {
        store_test_token_account(
            &mut self.svm,
            &self.vault_ata,
            &self.mint,
            &self.vault_state,
            amount,
        );
    }

    pub fn update_approver_member<F>(&mut self, approver_index: usize, update: F)
    where
        F: FnOnce(&mut Member),
    {
        let member_address = self.approvers[approver_index].member;

        let mut member_account = member(&self.svm, &member_address);

        update(&mut member_account);

        store_anchor_account(&mut self.svm, &member_address, &member_account);
    }

    pub fn update_policy<F>(&mut self, update: F)
    where
        F: FnOnce(&mut ApprovalPolicyVersion),
    {
        let mut policy = approval_policy_version(&self.svm, &self.approval_policy_version);

        update(&mut policy);

        store_anchor_account(&mut self.svm, &self.approval_policy_version, &policy);
    }

    pub fn update_recipient<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Recipient),
    {
        let mut recipient_account = recipient(&self.svm, &self.recipient);

        update(&mut recipient_account);

        store_anchor_account(&mut self.svm, &self.recipient, &recipient_account);
    }

    pub fn update_vault<F>(&mut self, update: F)
    where
        F: FnOnce(&mut VaultState),
    {
        let mut vault = vault_state(&self.svm, &self.vault_state);

        update(&mut vault);

        store_anchor_account(&mut self.svm, &self.vault_state, &vault);
    }

    pub fn update_payment<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Payment),
    {
        let mut payment = load_payment(&self.svm, &self.payment);

        update(&mut payment);

        store_anchor_account(&mut self.svm, &self.payment, &payment);
    }

    pub fn update_approval<F>(&mut self, approver_index: usize, update: F)
    where
        F: FnOnce(&mut Approval),
    {
        let approval_address = self.approval_pda(approver_index);

        let mut approval = load_approval(&self.svm, &approval_address);

        update(&mut approval);

        store_anchor_account(&mut self.svm, &approval_address, &approval);
    }

    pub fn finalize_with_witnesses(
        &mut self,
        witnesses: &[(Pubkey, Pubkey)],
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        execute_finalize_payment_approval(
            &solana_payout_platform::ID,
            &mut self.svm,
            &self.finalizer,
            &self.organization,
            &self.finalizer_member,
            &self.vault_state,
            &self.mint,
            &self.vault_ata,
            &self.recipient,
            &self.approval_policy_version,
            &self.payment,
            self.payment_id,
            witnesses,
        )
    }
}

fn find_test_ata(owner: &Pubkey, mint: &Pubkey) -> Pubkey {
    /*
     * Legacy ATA seeds:
     *
     * owner, SPL Token Program, mint
     */
    Pubkey::find_program_address(
        &[
            owner.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
    .0
}

fn store_test_mint(svm: &mut LiteSVM, mint: &Pubkey, decimals: u8) {
    /*
     * Legacy SPL Mint layout:
     *
     * mint_authority: 36 bytes
     * supply:          8 bytes
     * decimals:        1 byte
     * initialized:     1 byte
     * freeze_authority:36 bytes
     *
     * Total: 82 bytes
     */
    let mut data = vec![0u8; 82];

    data[36..44].copy_from_slice(&0u64.to_le_bytes());
    data[44] = decimals;
    data[45] = 1; // is_initialized = true

    svm.set_account(
        *mint,
        Account {
            lamports: TEST_ACCOUNT_LAMPORTS,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

fn store_test_token_account(
    svm: &mut LiteSVM,
    address: &Pubkey,
    mint: &Pubkey,
    token_owner: &Pubkey,
    amount: u64,
) {
    /*
     * Legacy SPL Token Account layout:
     *
     * mint:             bytes 0..32
     * owner:            bytes 32..64
     * amount:           bytes 64..72
     * delegate:         bytes 72..108
     * state:            byte 108
     * is_native:        bytes 109..121
     * delegated_amount: bytes 121..129
     * close_authority:  bytes 129..165
     */
    let mut data = vec![0u8; 165];

    data[0..32].copy_from_slice(mint.as_ref());
    data[32..64].copy_from_slice(token_owner.as_ref());
    data[64..72].copy_from_slice(&amount.to_le_bytes());

    // AccountState::Initialized
    data[108] = 1;

    svm.set_account(
        *address,
        Account {
            lamports: TEST_ACCOUNT_LAMPORTS,
            data,
            owner: anchor_spl::token::ID,
            executable: false,
            rent_epoch: 0,
        },
    )
    .unwrap();
}

pub fn setup_approval_fixture() -> ApprovalFixture {
    let program_id = solana_payout_platform::ID;
    let (mut svm, mut users) = setup_test_context(&program_id);

    let finalizer = users.remove("creator").expect("creator user must exist");

    let alice = users.remove("alice").expect("alice user must exist");

    let bob = users.remove("bob").expect("bob user must exist");

    let carol = users.remove("carol").expect("carol user must exist");

    let organization_id = 1;
    let finalizer_member_id = 1;
    let alice_member_id = 2;
    let bob_member_id = 3;
    let carol_member_id = 4;

    let recipient_id = 1;
    let vault_id = 1;
    let policy_id = 1;
    let policy_version = 1;
    let payment_id = 1;

    let (organization, organization_bump) =
        find_organization_pda(&program_id, &finalizer.pubkey(), organization_id);

    let (finalizer_member, finalizer_member_bump) =
        find_member_pda(&program_id, &organization, finalizer_member_id);

    let (alice_member, alice_member_bump) =
        find_member_pda(&program_id, &organization, alice_member_id);

    let (bob_member, bob_member_bump) = find_member_pda(&program_id, &organization, bob_member_id);

    let (carol_member, carol_member_bump) =
        find_member_pda(&program_id, &organization, carol_member_id);

    let (recipient, recipient_bump) = find_recipient_pda(&program_id, &organization, recipient_id);

    let (vault_state, vault_bump) = find_vault_pda(&program_id, &organization, vault_id);

    let (approval_policy_version, policy_bump) =
        find_policy_pda(&program_id, &organization, policy_id, policy_version);

    let (payment, payment_bump) = find_payment_pda(&program_id, &organization, payment_id);

    let mint = Pubkey::new_unique();
    let destination = Pubkey::new_unique();
    let vault_ata = find_test_ata(&vault_state, &mint);

    store_anchor_account(
        &mut svm,
        &organization,
        &Organization {
            organization_id,
            creator: finalizer.pubkey(),
            owner_member: finalizer_member,
            paused: false,
            bump: organization_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &finalizer_member,
        &Member {
            organization,
            member_id: finalizer_member_id,
            authorized_wallet: finalizer.pubkey(),
            authorization_revision: INITIAL_AUTHORIZATION_REVISION,
            roles: ROLE_ADMIN,
            active: true,
            bump: finalizer_member_bump,
        },
    );

    let test_approvers = [
        (&alice, alice_member, alice_member_id, alice_member_bump),
        (&bob, bob_member, bob_member_id, bob_member_bump),
        (&carol, carol_member, carol_member_id, carol_member_bump),
    ];

    for (user, member, member_id, bump) in test_approvers {
        store_anchor_account(
            &mut svm,
            &member,
            &Member {
                organization,
                member_id,
                authorized_wallet: user.pubkey(),
                authorization_revision: INITIAL_AUTHORIZATION_REVISION,
                roles: ROLE_APPROVER,
                active: true,
                bump,
            },
        );
    }

    store_anchor_account(
        &mut svm,
        &recipient,
        &Recipient {
            organization,
            recipient_id,
            current_destination: destination,
            wallet_revision: INITIAL_RECIPIENT_WALLET_REVISION,
            active: true,
            bump: recipient_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &vault_state,
        &VaultState {
            organization,
            vault_id,
            asset: Asset::Spl { mint },
            reserved_total: 0,
            active: true,
            bump: vault_bump,
        },
    );

    store_anchor_account(
        &mut svm,
        &approval_policy_version,
        &ApprovalPolicyVersion {
            organization,
            policy_id,
            version: policy_version,
            created_by_member: finalizer_member,
            eligible_members: vec![alice_member, bob_member, carol_member],
            threshold: 2,
            enabled: true,
            bump: policy_bump,
        },
    );

    let terms_hash = compute_payment_terms_hash(
        &organization,
        payment_id,
        INITIAL_PAYMENT_REVISION,
        &recipient,
        &destination,
        INITIAL_RECIPIENT_WALLET_REVISION,
        &vault_state,
        &mint,
        TEST_PAYMENT_AMOUNT,
        &approval_policy_version,
        SettlementRail::PublicSpl,
        0,
    );

    store_anchor_account(
        &mut svm,
        &payment,
        &Payment {
            organization,
            payment_id,
            created_by: finalizer_member,
            recipient,
            destination,
            recipient_wallet_revision: INITIAL_RECIPIENT_WALLET_REVISION,
            vault: vault_state,
            amount: TEST_PAYMENT_AMOUNT,
            policy_version: approval_policy_version,
            payment_revision: INITIAL_PAYMENT_REVISION,
            settlement_rail: SettlementRail::PublicSpl,
            execute_after: 0,
            terms_hash,
            payment_state: PaymentState::PendingApproval,
            reservation_state: ReservationState::None,
            bump: payment_bump,
        },
    );

    store_test_mint(&mut svm, &mint, TEST_USDC_DECIMALS);

    store_test_token_account(
        &mut svm,
        &vault_ata,
        &mint,
        &vault_state,
        TEST_VAULT_BALANCE,
    );

    ApprovalFixture {
        svm,
        finalizer,
        finalizer_member,
        organization,
        approvers: vec![
            TestApprover {
                user: alice,
                member: alice_member,
                member_id: alice_member_id,
                authorization_revision: INITIAL_AUTHORIZATION_REVISION,
            },
            TestApprover {
                user: bob,
                member: bob_member,
                member_id: bob_member_id,
                authorization_revision: INITIAL_AUTHORIZATION_REVISION,
            },
            TestApprover {
                user: carol,
                member: carol_member,
                member_id: carol_member_id,
                authorization_revision: INITIAL_AUTHORIZATION_REVISION,
            },
        ],
        recipient,
        vault_state,
        mint,
        vault_ata,
        approval_policy_version,
        payment,
        payment_id,
        payment_revision: INITIAL_PAYMENT_REVISION,
        payment_amount: TEST_PAYMENT_AMOUNT,
    }
}

pub struct ExecutePaymentFixture {
    /*
     * The approval fixture already contains the organization, payment,
     * vault, recipient and members. The execution fixture extends it with
     * the recipient's existing destination ATA.
     */
    pub approval: ApprovalFixture,

    pub destination_wallet: Pubkey,
    pub destination_ata: Pubkey,
}

impl ExecutePaymentFixture {
    pub fn execute(&mut self) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        execute_spl_payment_transaction(
            &solana_payout_platform::ID,
            &mut self.approval.svm,
            &self.approval.finalizer,
            &self.approval.organization,
            &self.approval.finalizer_member,
            &self.approval.mint,
            &self.approval.vault_state,
            &self.approval.vault_ata,
            &self.approval.recipient,
            &self.destination_ata,
            &self.approval.payment,
            self.approval.payment_id,
        )
    }

    pub fn update_executor_member<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Member),
    {
        let address = self.approval.finalizer_member;
        let mut executor_member = member(&self.approval.svm, &address);

        update(&mut executor_member);

        store_anchor_account(&mut self.approval.svm, &address, &executor_member);
    }

    pub fn update_organization<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Organization),
    {
        let address = self.approval.organization;
        let mut organization_account = organization(&self.approval.svm, &address);

        update(&mut organization_account);

        store_anchor_account(&mut self.approval.svm, &address, &organization_account);
    }

    pub fn set_execute_after(&mut self, execute_after: i64) {
        let mint = self.approval.mint;

        self.approval.update_payment(|payment| {
            payment.execute_after = execute_after;

            /*
             * This represents a legitimately constructed scheduled payment,
             * rather than corrupting execute_after without updating its
             * immutable terms hash.
             */
            payment.terms_hash = compute_payment_terms_hash(
                &payment.organization,
                payment.payment_id,
                payment.payment_revision,
                &payment.recipient,
                &payment.destination,
                payment.recipient_wallet_revision,
                &payment.vault,
                &mint,
                payment.amount,
                &payment.policy_version,
                SettlementRail::PublicSpl,
                payment.execute_after,
            );
        });
    }

    pub fn rotate_recipient_wallet(&mut self, new_destination_wallet: Pubkey) {
        self.approval.update_recipient(|recipient| {
            recipient.current_destination = new_destination_wallet;
            recipient.wallet_revision += 1;
        });

        let new_destination_ata = find_test_ata(&new_destination_wallet, &self.approval.mint);

        /*
         * The newly selected wallet has its proper ATA. Execution must still
         * fail because the approved Payment snapshot contains the old wallet
         * and old wallet revision.
         */
        store_test_token_account(
            &mut self.approval.svm,
            &new_destination_ata,
            &self.approval.mint,
            &new_destination_wallet,
            0,
        );

        self.destination_wallet = new_destination_wallet;
        self.destination_ata = new_destination_ata;
    }
}

pub fn setup_execute_payment_fixture() -> ExecutePaymentFixture {
    let mut approval = setup_approval_fixture();

    /*
     * The organization creator is already an admin/finalizer in the approval
     * fixture. Give that registered member the executor role as well.
     */
    let mut executor_member = member(&approval.svm, &approval.finalizer_member);

    executor_member.roles |= ROLE_EXECUTOR;

    store_anchor_account(
        &mut approval.svm,
        &approval.finalizer_member,
        &executor_member,
    );

    /*
     * Alice approves first. Bob then includes Alice's Approval + Member
     * witness, reaching the 2-of-3 threshold and reserving the payment amount.
     */
    approval.approve(0, &[]).unwrap();
    approval.approve(1, &[0]).unwrap();

    let recipient_account = recipient(&approval.svm, &approval.recipient);

    let destination_wallet = recipient_account.current_destination;

    let destination_ata = find_test_ata(&destination_wallet, &approval.mint);

    /*
     * Execution does not create the recipient ATA. Registration/client setup
     * must have created it before execution.
     */
    store_test_token_account(
        &mut approval.svm,
        &destination_ata,
        &approval.mint,
        &destination_wallet,
        0,
    );

    ExecutePaymentFixture {
        approval,
        destination_wallet,
        destination_ata,
    }
}

pub struct CancelPaymentFixture {
    pub approval: ApprovalFixture,
}

impl CancelPaymentFixture {
    pub fn cancel(&mut self) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        execute_cancel_payment(
            &solana_payout_platform::ID,
            &mut self.approval.svm,
            &self.approval.finalizer,
            &self.approval.organization,
            &self.approval.finalizer_member,
            &self.approval.vault_state,
            &self.approval.payment,
            self.approval.payment_id,
        )
    }

    /*
     * Calls cancellation using an approver wallet while still supplying
     * the real admin Member PDA. The wallet/member authorization check
     * must reject this combination.
     */
    pub fn cancel_as_approver(
        &mut self,
        approver_index: usize,
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let approver = &self.approval.approvers[approver_index];

        execute_cancel_payment(
            &solana_payout_platform::ID,
            &mut self.approval.svm,
            &approver.user,
            &self.approval.organization,
            &self.approval.finalizer_member,
            &self.approval.vault_state,
            &self.approval.payment,
            self.approval.payment_id,
        )
    }

    pub fn reach_approved_and_held(&mut self) {
        self.approval.approve(0, &[]).unwrap();
        self.approval.approve(1, &[0]).unwrap();

        let stored_payment = load_payment(&self.approval.svm, &self.approval.payment);

        assert_eq!(stored_payment.payment_state, PaymentState::Approved);

        assert_eq!(stored_payment.reservation_state, ReservationState::Held);
    }

    pub fn reach_awaiting_funds(&mut self) {
        self.approval
            .set_vault_token_balance(self.approval.payment_amount - 1);

        self.approval.approve(0, &[]).unwrap();
        self.approval.approve(1, &[0]).unwrap();

        let stored_payment = load_payment(&self.approval.svm, &self.approval.payment);

        assert_eq!(stored_payment.payment_state, PaymentState::AwaitingFunds);

        assert_eq!(stored_payment.reservation_state, ReservationState::None);
    }

    pub fn update_admin_member<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Member),
    {
        let address = self.approval.finalizer_member;
        let mut admin_member = member(&self.approval.svm, &address);

        update(&mut admin_member);

        store_anchor_account(&mut self.approval.svm, &address, &admin_member);
    }

    pub fn update_organization<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Organization),
    {
        let address = self.approval.organization;
        let mut organization_account = organization(&self.approval.svm, &address);

        update(&mut organization_account);

        store_anchor_account(&mut self.approval.svm, &address, &organization_account);
    }
}

pub fn setup_cancel_payment_fixture() -> CancelPaymentFixture {
    CancelPaymentFixture {
        approval: setup_approval_fixture(),
    }
}

pub struct RotateRecipientWalletFixture {
    /*
     * ExecutePaymentFixture gives us:
     *
     * - a valid organization and administrator
     * - a registered recipient
     * - a valid SPL vault and mint
     * - an Approved + Held payment
     * - the recipient's original destination ATA
     */
    pub execution: ExecutePaymentFixture,
    pub recipient_id: u64,

    pub submitted_mint: Pubkey,
    pub new_destination_wallet: Pubkey,
    pub new_destination_token_account: Pubkey,
}

impl RotateRecipientWalletFixture {
    pub fn rotate(&mut self) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        execute_rotate_recipient_wallet(
            &solana_payout_platform::ID,
            &mut self.execution.approval.svm,
            &self.execution.approval.finalizer,
            &self.execution.approval.organization,
            &self.execution.approval.finalizer_member,
            &self.execution.approval.vault_state,
            &self.execution.approval.recipient,
            &self.submitted_mint,
            &self.new_destination_token_account,
            self.recipient_id,
            self.new_destination_wallet,
        )
    }

    /*
     * Supplies an approver wallet while still passing the administrator's
     * Member PDA. The authorized-wallet constraint must reject it.
     */
    pub fn rotate_as_approver(
        &mut self,
        approver_index: usize,
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        let approver = &self.execution.approval.approvers[approver_index];

        execute_rotate_recipient_wallet(
            &solana_payout_platform::ID,
            &mut self.execution.approval.svm,
            &approver.user,
            &self.execution.approval.organization,
            &self.execution.approval.finalizer_member,
            &self.execution.approval.vault_state,
            &self.execution.approval.recipient,
            &self.submitted_mint,
            &self.new_destination_token_account,
            self.recipient_id,
            self.new_destination_wallet,
        )
    }

    pub fn prepare_destination(&mut self, destination_wallet: Pubkey) {
        let destination_token_account = find_test_ata(&destination_wallet, &self.submitted_mint);

        store_test_token_account(
            &mut self.execution.approval.svm,
            &destination_token_account,
            &self.submitted_mint,
            &destination_wallet,
            0,
        );

        self.new_destination_wallet = destination_wallet;
        self.new_destination_token_account = destination_token_account;
    }

    pub fn use_noncanonical_destination_token_account(&mut self) {
        let noncanonical_token_account = Pubkey::new_unique();

        store_test_token_account(
            &mut self.execution.approval.svm,
            &noncanonical_token_account,
            &self.submitted_mint,
            &self.new_destination_wallet,
            0,
        );

        self.new_destination_token_account = noncanonical_token_account;
    }

    pub fn use_wrong_mint(&mut self) {
        let wrong_mint = Pubkey::new_unique();

        store_test_mint(
            &mut self.execution.approval.svm,
            &wrong_mint,
            TEST_USDC_DECIMALS,
        );

        self.submitted_mint = wrong_mint;

        let destination_token_account = find_test_ata(&self.new_destination_wallet, &wrong_mint);

        store_test_token_account(
            &mut self.execution.approval.svm,
            &destination_token_account,
            &wrong_mint,
            &self.new_destination_wallet,
            0,
        );

        self.new_destination_token_account = destination_token_account;
    }

    pub fn corrupt_destination_token_owner(&mut self, wrong_owner: Pubkey) {
        store_test_token_account(
            &mut self.execution.approval.svm,
            &self.new_destination_token_account,
            &self.submitted_mint,
            &wrong_owner,
            0,
        );
    }

    pub fn update_registrar_member<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Member),
    {
        let address = self.execution.approval.finalizer_member;

        let mut member_account = member(&self.execution.approval.svm, &address);

        update(&mut member_account);

        store_anchor_account(&mut self.execution.approval.svm, &address, &member_account);
    }

    pub fn update_organization<F>(&mut self, update: F)
    where
        F: FnOnce(&mut Organization),
    {
        let address = self.execution.approval.organization;

        let mut organization_account = organization(&self.execution.approval.svm, &address);

        update(&mut organization_account);

        store_anchor_account(
            &mut self.execution.approval.svm,
            &address,
            &organization_account,
        );
    }

    /*
     * Execute using the newly rotated destination ATA.
     *
     * The ATA itself is valid, but the existing Payment still contains the
     * previous wallet and previous wallet revision, so execution must fail.
     */
    pub fn execute_existing_payment_against_new_destination(
        &mut self,
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        self.execution.destination_ata = self.new_destination_token_account;

        self.execution.execute()
    }

    pub fn cancel_existing_payment(
        &mut self,
    ) -> Result<TransactionMetadata, FailedTransactionMetadata> {
        execute_cancel_payment(
            &solana_payout_platform::ID,
            &mut self.execution.approval.svm,
            &self.execution.approval.finalizer,
            &self.execution.approval.organization,
            &self.execution.approval.finalizer_member,
            &self.execution.approval.vault_state,
            &self.execution.approval.payment,
            self.execution.approval.payment_id,
        )
    }
}

pub fn setup_rotate_recipient_wallet_fixture() -> RotateRecipientWalletFixture {
    let mut execution = setup_execute_payment_fixture();

    let recipient_id =
        recipient(&execution.approval.svm, &execution.approval.recipient).recipient_id;

    /*
     * Use Alice's wallet as the new payout destination. Her role as an
     * approver is unrelated to being a token recipient; it simply provides
     * a stable test wallet address.
     */
    let new_destination_wallet = execution.approval.approvers[0].user.pubkey();

    let submitted_mint = execution.approval.mint;

    let new_destination_token_account = find_test_ata(&new_destination_wallet, &submitted_mint);

    store_test_token_account(
        &mut execution.approval.svm,
        &new_destination_token_account,
        &submitted_mint,
        &new_destination_wallet,
        0,
    );

    RotateRecipientWalletFixture {
        execution,
        submitted_mint,
        new_destination_wallet,
        new_destination_token_account,
        recipient_id,
    }
}
