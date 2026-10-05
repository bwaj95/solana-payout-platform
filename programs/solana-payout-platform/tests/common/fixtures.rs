use std::collections::HashMap;

use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_pubkey::Pubkey;

use crate::common::users::User;

pub const HUNDRED_SOL: u64 = 100_000_000_000u64;

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
