#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleStatus {
    Draft,
    Provisioning,
    Active,
    Archived,
}

impl LifecycleStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransactionSubmissionStatus {
    Prepared,
    Submitted,
    Unknown,
    Confirmed,
    Finalized,
    Failed,
    Expired,
}

impl TransactionSubmissionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Prepared => "prepared",
            Self::Submitted => "submitted",
            Self::Unknown => "unknown",
            Self::Confirmed => "confirmed",
            Self::Finalized => "finalized",
            Self::Failed => "failed",
            Self::Expired => "expired",
        }
    }
}

/*
 * These values must remain aligned with the Anchor program's role bitmask.
 */
pub const ROLE_OWNER: i32 = 1 << 0;
pub const ROLE_ADMIN: i32 = 1 << 1;
pub const ROLE_PREPARER: i32 = 1 << 2;
pub const ROLE_APPROVER: i32 = 1 << 3;
pub const ROLE_EXECUTOR: i32 = 1 << 4;
pub const ROLE_FINANCE: i32 = 1 << 5;
pub const ROLE_TREASURY: i32 = 1 << 6;

pub const INITIAL_OWNER_ROLES: i32 = ROLE_OWNER | ROLE_ADMIN;

pub const ASSIGNABLE_MEMBER_ROLES: i32 =
    ROLE_ADMIN | ROLE_PREPARER | ROLE_APPROVER | ROLE_EXECUTOR | ROLE_FINANCE | ROLE_TREASURY;
