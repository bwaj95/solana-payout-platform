use solana_payout_platform::{MEMBER_SEED, ORGANIZATION_SEED};
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
