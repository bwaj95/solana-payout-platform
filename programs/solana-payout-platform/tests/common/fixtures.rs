use std::collections::HashMap;

use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_pubkey::Pubkey;

use crate::common::users::User;
use anchor_lang::AccountSerialize;
use solana_account::Account;
use solana_payout_platform::{
    compute_payment_terms_hash, ApprovalPolicyVersion, Asset, Member, Organization, Recipient,
    SettlementRail, VaultState, INITIAL_AUTHORIZATION_REVISION, INITIAL_RECIPIENT_WALLET_REVISION,
    ROLE_PREPARER,
};

use crate::common::pda::{
    find_member_pda, find_organization_pda, find_policy_pda, find_recipient_pda, find_vault_pda,
};

use crate::common::{
    accounts::{approval_policy_version, member, organization, recipient, vault_state},
    executor::create_payment,
};

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
