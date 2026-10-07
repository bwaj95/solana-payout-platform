use anchor_lang::prelude::*;

use crate::MAX_POLICY_MEMBERS;

#[account]
#[derive(InitSpace)]
pub struct Organization {
    pub organization_id: u64,
    pub creator: Pubkey,
    pub owner_member: Pubkey,
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Member {
    pub organization: Pubkey,
    pub member_id: u64,

    pub authorized_wallet: Pubkey,
    pub authorization_revision: u64,

    pub roles: u16,
    pub active: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct MemberWallet {
    pub organization: Pubkey,
    pub member: Pubkey,
    pub authorized_wallet: Pubkey,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct ApprovalPolicyVersion {
    pub organization: Pubkey,

    pub policy_id: u64,
    pub version: u64,
    pub created_by_member: Pubkey,

    #[max_len(MAX_POLICY_MEMBERS)]
    pub eligible_members: Vec<Pubkey>,
    pub threshold: u8,

    pub enabled: bool,

    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum Asset {
    NativeSol,
    Spl { mint: Pubkey },
}

#[account]
#[derive(InitSpace)]
pub struct VaultState {
    pub organization: Pubkey,
    pub vault_id: u64,
    pub asset: Asset,
    pub reserved_total: u64,
    pub active: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Recipient {
    pub organization: Pubkey,
    pub recipient_id: u64,
    pub current_destination: Pubkey,
    pub wallet_revision: u32,
    pub active: bool,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum PaymentState {
    PendingApproval,
    AwaitingFunds,
    Approved,
    Paid,
    Cancelled,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum ReservationState {
    None,
    Held,
    Consumed,
    Released,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq, Eq, InitSpace)]
pub enum SettlementRail {
    PublicSol,
    PublicSpl,
    PrivateMagicBlock,
}

impl SettlementRail {
    // Explicit hash tags prevent the hash format from depending implicitly
    // on how Anchor serializes enum variants.
    pub fn hash_tag(self) -> u8 {
        match self {
            Self::PublicSol => 0,
            Self::PublicSpl => 1,
            Self::PrivateMagicBlock => 2,
        }
    }
}

#[account]
#[derive(InitSpace)]
pub struct Payment {
    pub organization: Pubkey,
    pub payment_id: u64,

    // Stable Member PDA, not the authority wallet.
    pub created_by: Pubkey,

    pub recipient: Pubkey,
    pub destination: Pubkey,
    pub recipient_wallet_revision: u32,

    pub vault: Pubkey,
    pub amount: u64,

    // Exact immutable ApprovalPolicyVersion PDA.
    pub policy_version: Pubkey,

    pub payment_revision: u32,
    pub settlement_rail: SettlementRail,
    pub execute_after: i64,

    pub terms_hash: [u8; 32],

    pub payment_state: PaymentState,
    pub reservation_state: ReservationState,

    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Approval {
    pub organization: Pubkey,
    pub payment: Pubkey,
    pub payment_revision: u32,
    pub policy: Pubkey,

    pub member: Pubkey,
    pub member_authorization_revision: u64,
    pub authorization_wallet: Pubkey,

    pub approved_at: i64,
    pub terms_hash: [u8; 32],

    pub bump: u8,
}
