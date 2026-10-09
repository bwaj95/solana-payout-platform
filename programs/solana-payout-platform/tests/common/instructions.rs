use anchor_lang::{
    solana_program::instruction::AccountMeta, system_program, InstructionData, ToAccountMetas,
};
use solana_message::Instruction;
use solana_payout_platform::{
    accounts::{self, CreatePayment, InitializeVault},
    instruction,
};
use solana_pubkey::Pubkey;

use crate::common::pda::{
    find_member_pda, find_member_wallet_pda, find_organization_pda, find_policy_version_pda,
    find_vault_state_pda, find_vault_token_account,
};

use solana_payout_platform::accounts::{
    ApprovePayment, CancelPayment, ExecuteSplPayment, FinalizePaymentApproval,
    RotateRecipientWallet,
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

pub fn initialize_vault_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    admin_member: &Pubkey,
    mint: &Pubkey,
    vault_id: u64,
) -> Instruction {
    let (vault_state, _) = find_vault_state_pda(program_id, organization, vault_id);

    let vault_token_account = find_vault_token_account(&vault_state, mint);

    let accounts = InitializeVault {
        authority: *authority,
        organization: *organization,
        admin_member: *admin_member,
        vault_state,
        mint: *mint,
        vault_token_account,
        token_program: anchor_spl::token::ID,
        associated_token_program: anchor_spl::associated_token::ID,
        system_program: anchor_lang::system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::InitializeVault { vault_id }.data(),
    }
}

pub fn create_payment_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    preparer_member: &Pubkey,
    recipient: &Pubkey,
    vault: &Pubkey,
    approval_policy_version: &Pubkey,
    payment_id: u64,
    amount: u64,
    execute_after: i64,
) -> Instruction {
    let (payment, _) = crate::common::pda::find_payment_pda(program_id, organization, payment_id);

    let accounts = CreatePayment {
        authority: *authority,
        organization: *organization,
        preparer_member: *preparer_member,
        recipient: *recipient,
        vault_state: *vault,
        approval_policy_version: *approval_policy_version,
        payment,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::CreatePayment {
            payment_id,
            amount,
            execute_after,
        }
        .data(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn approve_payment_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
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
) -> Instruction {
    let mut accounts = ApprovePayment {
        authority: *authority,
        organization: *organization,
        approver_member: *approver_member,
        vault_state: *vault_state,
        mint: *mint,
        vault_ata: *vault_ata,
        recipient: *recipient,
        approval_policy_version: *approval_policy_version,
        payment: *payment,
        approval: *approval,
        system_program: system_program::ID,
    }
    .to_account_metas(None);

    // remaining_accounts must be ordered as:
    // approval_1, member_1, approval_2, member_2, ...
    for (approval, member) in witnesses {
        accounts.push(AccountMeta::new_readonly(*approval, false));
        accounts.push(AccountMeta::new_readonly(*member, false));
    }

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::ApprovePayment { payment_id }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn finalize_payment_approval_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
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
) -> Instruction {
    let mut accounts = FinalizePaymentApproval {
        authority: *authority,
        organization: *organization,
        finalizer_member: *finalizer_member,
        vault_state: *vault_state,
        mint: *mint,
        vault_ata: *vault_ata,
        recipient: *recipient,
        approval_policy_version: *approval_policy_version,
        payment: *payment,
    }
    .to_account_metas(None);

    for (approval, member) in witnesses {
        accounts.push(AccountMeta::new_readonly(*approval, false));
        accounts.push(AccountMeta::new_readonly(*member, false));
    }

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::FinalizePaymentApproval { payment_id }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn execute_spl_payment_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    executor_member: &Pubkey,
    mint: &Pubkey,
    vault_state: &Pubkey,
    vault_ata: &Pubkey,
    recipient: &Pubkey,
    destination_ata: &Pubkey,
    payment: &Pubkey,
    payment_id: u64,
) -> Instruction {
    let accounts = ExecuteSplPayment {
        authority: *authority,
        organization: *organization,
        executor_member: *executor_member,
        mint: *mint,
        vault_state: *vault_state,
        vault_ata: *vault_ata,
        recipient: *recipient,
        destination_ata: *destination_ata,
        payment: *payment,
        token_program: anchor_spl::token::ID,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::ExecuteSplPayment { payment_id }.data(),
    }
}

pub fn cancel_payment_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    admin_member: &Pubkey,
    vault_state: &Pubkey,
    payment: &Pubkey,
    payment_id: u64,
) -> Instruction {
    let accounts = CancelPayment {
        authority: *authority,
        organization: *organization,
        admin_member: *admin_member,
        vault_state: *vault_state,
        payment: *payment,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::CancelPayment { payment_id }.data(),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn rotate_recipient_wallet_ix(
    program_id: &Pubkey,
    authority: &Pubkey,
    organization: &Pubkey,
    registrar_member: &Pubkey,
    vault_state: &Pubkey,
    recipient: &Pubkey,
    mint: &Pubkey,
    new_destination_token_account: &Pubkey,
    recipient_id: u64,
    new_destination_wallet: Pubkey,
) -> Instruction {
    let accounts = RotateRecipientWallet {
        authority: *authority,
        organization: *organization,
        registrar_member: *registrar_member,
        vault_state: *vault_state,
        recipient: *recipient,
        mint: *mint,
        new_destination_token_account: *new_destination_token_account,
    }
    .to_account_metas(None);

    Instruction {
        program_id: *program_id,
        accounts,
        data: instruction::RotateRecipientWallet {
            recipient_id,
            new_destination_wallet,
        }
        .data(),
    }
}
