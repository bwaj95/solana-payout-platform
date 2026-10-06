use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_message::{Instruction, Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use crate::common::{
    instructions::{create_member_ix, create_policy_version_ix, initialize_organization_ix},
    users::User,
};

///  Builds and submits a transaction to LiteSVM.
pub fn execute_transaction(
    svm: &mut LiteSVM,
    payer: &Pubkey,
    signers: &[&dyn Signer],
    ixs: &[Instruction],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let blockhash = svm.latest_blockhash();
    let msg = Message::new_with_blockhash(ixs, Some(payer), &blockhash);
    let tx = VersionedTransaction::try_new(VersionedMessage::Legacy(msg), signers).unwrap();

    svm.send_transaction(tx)
}

pub fn initialize_organization(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    creator: &User,
    organization_id: u64,
    owner_member_id: u64,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = initialize_organization_ix(
        program_id,
        &creator.pubkey(),
        organization_id,
        owner_member_id,
    );

    let payer = creator.pubkey();
    let signer = creator.signer();

    execute_transaction(svm, &payer, &[signer], &[ix])
}

pub fn create_member(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    admin_member: &Pubkey,
    member_id: u64,
    authorized_wallet: &Pubkey,
    roles: u16,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = create_member_ix(
        program_id,
        &authority.pubkey(),
        organization,
        admin_member,
        member_id,
        authorized_wallet,
        roles,
    );

    execute_transaction(svm, &authority.pubkey(), &[authority.signer()], &[ix])
}

pub fn create_policy_version(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    admin_member: &Pubkey,
    policy_id: u64,
    version: u64,
    threshold: u8,
    eligible_members: &[Pubkey],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = create_policy_version_ix(
        program_id,
        &authority.pubkey(),
        organization,
        admin_member,
        policy_id,
        version,
        threshold,
        eligible_members,
    );

    execute_transaction(svm, &authority.pubkey(), &[authority.signer()], &[ix])
}
