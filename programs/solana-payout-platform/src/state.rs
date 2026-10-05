use anchor_lang::prelude::*;

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
