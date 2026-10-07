use litesvm::{
    types::{FailedTransactionMetadata, TransactionMetadata},
    LiteSVM,
};
use solana_message::{Instruction, Message, VersionedMessage};
use solana_pubkey::Pubkey;
use solana_signer::Signer;
use solana_transaction::versioned::VersionedTransaction;

use crate::common::instructions::{approve_payment_ix, finalize_payment_approval_ix};
use crate::common::{
    instructions::{
        create_member_ix, create_payment_ix, create_policy_version_ix, initialize_organization_ix,
        initialize_vault_ix,
    },
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

pub fn initialize_vault(
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

#[allow(clippy::too_many_arguments)]
pub fn create_payment(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    preparer_member: &Pubkey,
    recipient: &Pubkey,
    vault: &Pubkey,
    approval_policy_version: &Pubkey,
    payment_id: u64,
    amount: u64,
    execute_after: i64,
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = create_payment_ix(
        program_id,
        &authority.pubkey(),
        organization,
        preparer_member,
        recipient,
        vault,
        approval_policy_version,
        payment_id,
        amount,
        execute_after,
    );

    let payer = authority.pubkey();

    execute_transaction(svm, &payer, &[authority.signer()], &[ix])
}

#[allow(clippy::too_many_arguments)]
pub fn approve_payment(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    approver_member: &Pubkey,
    vault_state: &Pubkey,
    mint: &Pubkey,
    vault_ata: &Pubkey,
    recipient: &Pubkey,
    approval_policy_version: &Pubkey,
    payment: &Pubkey,
    approval: &Pubkey,
    payment_id: u64,
    witnesses: &[(Pubkey, Pubkey)],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = approve_payment_ix(
        program_id,
        &authority.pubkey(),
        organization,
        approver_member,
        vault_state,
        mint,
        vault_ata,
        recipient,
        approval_policy_version,
        payment,
        approval,
        payment_id,
        witnesses,
    );

    execute_transaction(svm, &authority.pubkey(), &[authority.signer()], &[ix])
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_payment_approval(
    program_id: &Pubkey,
    svm: &mut LiteSVM,
    authority: &User,
    organization: &Pubkey,
    finalizer_member: &Pubkey,
    vault_state: &Pubkey,
    mint: &Pubkey,
    vault_ata: &Pubkey,
    recipient: &Pubkey,
    approval_policy_version: &Pubkey,
    payment: &Pubkey,
    payment_id: u64,
    witnesses: &[(Pubkey, Pubkey)],
) -> Result<TransactionMetadata, FailedTransactionMetadata> {
    let ix = finalize_payment_approval_ix(
        program_id,
        &authority.pubkey(),
        organization,
        finalizer_member,
        vault_state,
        mint,
        vault_ata,
        recipient,
        approval_policy_version,
        payment,
        payment_id,
        witnesses,
    );

    execute_transaction(svm, &authority.pubkey(), &[authority.signer()], &[ix])
}
