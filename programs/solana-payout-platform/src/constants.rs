use anchor_lang::prelude::*;

#[constant]

pub const ORGANIZATION_SEED: &[u8] = b"organization";
pub const MEMBER_SEED: &[u8] = b"member";
pub const MEMBER_WALLET_SEED: &[u8] = b"member_wallet";
pub const VAULT_SEED: &[u8] = b"vault";
pub const RECIPIENT_SEED: &[u8] = b"recipient";
pub const PAYMENT_SEED: &[u8] = b"payment";
pub const APPROVAL_SEED: &[u8] = b"approval";

pub const INITIAL_AUTHORIZATION_REVISION: u64 = 1;
pub const INITIAL_RECIPIENT_WALLET_REVISION: u32 = 1;
pub const INITIAL_PAYMENT_REVISION: u32 = 1;

// A bitmask lets one Member hold multiple roles without storing a variable-length vector.
pub const ROLE_OWNER: u16 = 1 << 0;
pub const ROLE_ADMIN: u16 = 1 << 1;
pub const ROLE_PREPARER: u16 = 1 << 2;
pub const ROLE_APPROVER: u16 = 1 << 3;
pub const ROLE_EXECUTOR: u16 = 1 << 4;
pub const ROLE_FINANCE: u16 = 1 << 5;
pub const ROLE_TREASURY: u16 = 1 << 6;

pub const INITIAL_OWNER_ROLES: u16 = ROLE_OWNER | ROLE_ADMIN;

// ASSIGNABLE_MEMBER_ROLES should exclude ROLE_OWNER
pub const ASSIGNABLE_MEMBER_ROLES: u16 =
    ROLE_ADMIN | ROLE_PREPARER | ROLE_APPROVER | ROLE_EXECUTOR | ROLE_FINANCE | ROLE_TREASURY;

pub const POLICY_SEED: &[u8] = b"policy";

// For each approval we need to provide the Member-Approval account pair. 8 is a reasonable upper bound.
pub const MAX_POLICY_MEMBERS: u8 = 8;

// Never change this after approvals have been created.
// It prevents this hash from being confused with another hash used elsewhere.
pub const PAYMENT_TERMS_DOMAIN: &[u8] = b"solana-payout-platform:payment-terms:v1";
