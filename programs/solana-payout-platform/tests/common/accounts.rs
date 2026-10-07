use anchor_lang::AccountDeserialize;
use litesvm::LiteSVM;
use solana_payout_platform::{
    Approval, ApprovalPolicyVersion, Member, MemberWallet, Organization, Payment, Recipient,
    VaultState,
};
use solana_pubkey::Pubkey;

pub fn organization(svm: &LiteSVM, organization: &Pubkey) -> Organization {
    let account = svm
        .get_account(organization)
        .expect("Organization not found");

    let mut data: &[u8] = &account.data;

    Organization::try_deserialize(&mut data).unwrap()
}

pub fn member(svm: &LiteSVM, member: &Pubkey) -> Member {
    let account = svm.get_account(member).expect("Member not found");

    let mut data: &[u8] = &account.data;

    Member::try_deserialize(&mut data).unwrap()
}

pub fn member_wallet(svm: &LiteSVM, member_wallet: &Pubkey) -> MemberWallet {
    let account = svm
        .get_account(member_wallet)
        .expect("MemberWallet not found");

    let mut data: &[u8] = &account.data;

    MemberWallet::try_deserialize(&mut data).unwrap()
}

pub fn approval_policy_version(svm: &LiteSVM, policy_version: &Pubkey) -> ApprovalPolicyVersion {
    let account = svm
        .get_account(policy_version)
        .expect("ApprovalPolicyVersion not found");

    let mut data: &[u8] = &account.data;

    ApprovalPolicyVersion::try_deserialize(&mut data).unwrap()
}

pub fn get_vault_state(svm: &LiteSVM, vault_state: &Pubkey) -> VaultState {
    let account = svm
        .get_account(vault_state)
        .expect("VaultState account not found");

    let mut data: &[u8] = &account.data;

    VaultState::try_deserialize(&mut data).unwrap()
}

pub fn payment(svm: &LiteSVM, payment: &Pubkey) -> Payment {
    let account = svm.get_account(payment).expect("Payment account not found");

    let mut data: &[u8] = &account.data;

    Payment::try_deserialize(&mut data).unwrap()
}

pub fn recipient(svm: &LiteSVM, address: &Pubkey) -> Recipient {
    let account = svm
        .get_account(address)
        .expect("Recipient account not found");

    let mut data: &[u8] = &account.data;

    Recipient::try_deserialize(&mut data).unwrap()
}

pub fn vault_state(svm: &LiteSVM, address: &Pubkey) -> VaultState {
    let account = svm
        .get_account(address)
        .expect("VaultState account not found");

    let mut data: &[u8] = &account.data;

    VaultState::try_deserialize(&mut data).unwrap()
}

pub fn approval(svm: &LiteSVM, address: &Pubkey) -> Approval {
    let account = svm
        .get_account(address)
        .expect("Approval account not found");

    let mut data: &[u8] = &account.data;

    Approval::try_deserialize(&mut data).unwrap()
}
