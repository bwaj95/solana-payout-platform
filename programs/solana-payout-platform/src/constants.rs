use anchor_lang::prelude::*;

#[constant]

pub const ORGANIZATION_SEED: &[u8] = b"organization";
pub const MEMBER_SEED: &[u8] = b"member";

pub const INITIAL_AUTHORIZATION_REVISION: u64 = 1;

// A bitmask lets one Member hold multiple roles without storing a variable-length vector.
pub const ROLE_OWNER: u16 = 1 << 0;
pub const ROLE_ADMIN: u16 = 1 << 1;

pub const INITIAL_OWNER_ROLES: u16 = ROLE_OWNER | ROLE_ADMIN;
