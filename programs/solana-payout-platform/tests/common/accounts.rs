use anchor_lang::AccountDeserialize;
use litesvm::LiteSVM;
use solana_payout_platform::{Member, Organization};
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
