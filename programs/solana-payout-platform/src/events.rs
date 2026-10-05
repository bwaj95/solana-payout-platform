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
