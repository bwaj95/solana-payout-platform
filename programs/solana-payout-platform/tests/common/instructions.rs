use anchor_lang::{
    solana_program::instruction::AccountMeta, system_program, InstructionData, ToAccountMetas,
};
use solana_message::Instruction;
use solana_payout_platform::{accounts, instruction};
use solana_pubkey::Pubkey;

use crate::common::pda::{
    find_member_pda, find_member_wallet_pda, find_organization_pda, find_policy_version_pda,
};

pub fn initialize_organization_ix(
    program_id: &Pubkey,
    creator: &Pubkey,
    organization_id: u64,
    owner_member_id: u64,
) -> Instruction {
    let (organization, _) = find_organization_pda(program_id, creator, organization_id);

    let (owner_member, _) = find_member_pda(program_id, &organization, owner_member_id);

    let (owner_member_wallet, _) = find_member_wallet_pda(program_id, &organization, creator);

    initialize_organization_ix_with_accounts(
        program_id,
        creator,
        &organization,
        &owner_member,
        &owner_member_wallet,
        organization_id,
        owner_member_id,
    )
}

pub fn initialize_organization_ix_with_accounts(
    program_id: &Pubkey,
    creator: &Pubkey,
    organization: &Pubkey,
    owner_member: &Pubkey,
    owner_member_wallet: &Pubkey,
    organization_id: u64,
    owner_member_id: u64,
) -> Instruction {
    let accounts = accounts::InitializeOrganization {
        creator: *creator,
        organization: *organization,
        owner_member: *owner_member,
        owner_member_wallet: *owner_member_wallet,
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

pub fn create_member_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    admin_member: &Pubkey,
    member_id: u64,
    authorized_wallet: &Pubkey,
    roles: u16,
) -> Instruction {
    let (member, _) = find_member_pda(program_id, organization, member_id);

    let (member_wallet, _) = find_member_wallet_pda(program_id, organization, authorized_wallet);

    let accounts = accounts::CreateMember {
        authority: *authority,
        organization: *organization,
        admin_member: *admin_member,
        member,
        member_wallet,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::CreateMember {
            member_id,
            authorized_wallet: *authorized_wallet,
            roles,
        }
        .data(),
    }
}

pub fn create_policy_version_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    admin_member: &Pubkey,
    policy_id: u64,
    version: u64,
    threshold: u8,
    eligible_members: &[Pubkey],
) -> Instruction {
    let (approval_policy_version, _) =
        find_policy_version_pda(program_id, organization, policy_id, version);

    let mut account_metas = accounts::CreatePolicyVersion {
        authority: *authority,
        organization: *organization,
        admin_member: *admin_member,
        approval_policy_version,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    /*
     * Accounts appended after the declared Anchor accounts become
     * ctx.remaining_accounts inside the handler.
     */
    account_metas.extend(
        eligible_members
            .iter()
            .map(|member| AccountMeta::new_readonly(*member, false)),
    );

    Instruction {
        program_id: *program_id,
        accounts: account_metas,
        data: instruction::CreatePolicyVersion {
            policy_id,
            version,
            threshold,
        }
        .data(),
    }
}
