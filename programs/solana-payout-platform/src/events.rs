use anchor_lang::prelude::*;

#[event]
pub struct OrganizationInitialized {
    pub organization: Pubkey,
    pub organization_id: u64,
    pub creator: Pubkey,
    pub owner_member: Pubkey,
    pub owner_member_id: u64,
}
