use solana_payout_platform::{MEMBER_SEED, MEMBER_WALLET_SEED, ORGANIZATION_SEED, POLICY_SEED, VAULT_SEED};
use solana_pubkey::Pubkey;

pub fn find_organization_pda(
    program_id: &Pubkey,
    creator: &Pubkey,
    organization_id: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            ORGANIZATION_SEED,
            creator.as_ref(),
            organization_id.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

pub fn find_member_pda(program_id: &Pubkey, organization: &Pubkey, member_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            MEMBER_SEED,
            organization.as_ref(),
            member_id.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

pub fn find_member_wallet_pda(
    program_id: &Pubkey,
    organization: &Pubkey,
    authorized_wallet: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            MEMBER_WALLET_SEED,
            organization.as_ref(),
            authorized_wallet.as_ref(),
        ],
        program_id,
    )
}

pub fn find_policy_version_pda(
    program_id: &Pubkey,
    organization: &Pubkey,
    policy_id: u64,
    version: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            POLICY_SEED,
            organization.as_ref(),
            policy_id.to_le_bytes().as_ref(),
            version.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

pub fn find_vault_state_pda(program_id: &Pubkey, organization: &Pubkey, vault_id: u64) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            VAULT_SEED,
            organization.as_ref(),
            vault_id.to_le_bytes().as_ref(),
        ],
        program_id,
    )
}

pub fn find_vault_token_account(vault_state: &Pubkey, mint: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[
            vault_state.as_ref(),
            anchor_spl::token::ID.as_ref(),
            mint.as_ref(),
        ],
        &anchor_spl::associated_token::ID,
    )
    .0
}
