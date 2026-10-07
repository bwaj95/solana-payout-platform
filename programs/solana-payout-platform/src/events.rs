use crate::PaymentState;
use anchor_lang::prelude::*;

#[event]
pub struct OrganizationInitialized {
    pub organization: Pubkey,
    pub organization_id: u64,
    pub creator: Pubkey,
    pub owner_member: Pubkey,
    pub owner_member_id: u64,
}

#[event]
pub struct MemberCreated {
    pub organization: Pubkey,
    pub member: Pubkey,
    pub member_wallet: Pubkey,
    pub member_id: u64,
    pub authorized_wallet: Pubkey,
    pub roles: u16,
    pub created_by_member: Pubkey,
    pub created_by_wallet: Pubkey,
}

#[event]
pub struct PolicyVersionCreated {
    pub organization: Pubkey,
    pub policy_version: Pubkey,
    pub policy_id: u64,
    pub version: u64,
    pub threshold: u8,
    pub eligible_member_count: u8,
    pub created_by_member: Pubkey,
    pub created_by_wallet: Pubkey,
}

#[event]
pub struct PaymentApprovalRecorded {
    pub organization: Pubkey,
    pub payment: Pubkey,
    pub approval: Pubkey,

    pub approver_member: Pubkey,
    pub authorization_wallet: Pubkey,

    pub payment_revision: u32,
    pub member_authorization_revision: u64,

    pub policy_version: Pubkey,
    pub terms_hash: [u8; 32],

    pub approved_at: i64,
}

#[event]
pub struct PaymentApprovalThresholdReached {
    pub organization: Pubkey,
    pub payment: Pubkey,
    pub policy_version: Pubkey,
    pub vault: Pubkey,

    // The member whose action caused this threshold evaluation.
    // For recovery finalization, this is the finalizer member.
    pub triggered_by_member: Pubkey,

    pub valid_approval_count: u8,
    pub threshold: u8,

    pub amount: u64,
    pub funds_reserved: bool,
    pub vault_reserved_total: u64,

    pub payment_state: PaymentState,
    pub processed_at: i64,
}
