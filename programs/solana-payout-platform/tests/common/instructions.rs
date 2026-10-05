use anchor_lang::{system_program, InstructionData, ToAccountMetas};
use solana_message::Instruction;
use solana_payout_platform::{accounts::InitializeOrganization, instruction};
use solana_pubkey::Pubkey;

use crate::common::pda::{find_member_pda, find_organization_pda};

pub fn initialize_organization_ix(
    program_id: &Pubkey,
    creator: &Pubkey,
    organization_id: u64,
    owner_member_id: u64,
) -> Instruction {
    let (organization, _) = find_organization_pda(program_id, creator, organization_id);

    let (owner_member, _) = find_member_pda(program_id, &organization, owner_member_id);

    initialize_organization_ix_with_accounts(
        program_id,
        creator,
        &organization,
        &owner_member,
        organization_id,
        owner_member_id,
    )
}

/// Allows negative tests to deliberately supply invalid accounts.
pub fn initialize_organization_ix_with_accounts(
    program_id: &Pubkey,
    creator: &Pubkey,
    organization: &Pubkey,
    owner_member: &Pubkey,
    organization_id: u64,
    owner_member_id: u64,
) -> Instruction {
    let accounts = InitializeOrganization {
        creator: *creator,
        organization: *organization,
        owner_member: *owner_member,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::InitializeOrganization {
            organization_id,
            owner_member_id,
        }
        .data(),
    }
}
