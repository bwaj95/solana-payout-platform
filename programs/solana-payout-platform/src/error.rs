use anchor_lang::prelude::*;

#[error_code]
pub enum ErrorCode {
    #[msg("The administrative member does not belong to this organization")]
    MemberOrganizationMismatch,

    #[msg("The administrative member is inactive")]
    InactiveMember,

    #[msg("The signer is not the member's authorized wallet")]
    UnauthorizedWallet,

    #[msg("The member does not have the administrator role")]
    MissingAdminRole,

    #[msg("The supplied member roles are invalid")]
    InvalidMemberRoles,

    #[msg("The authorized wallet cannot be the default public key")]
    InvalidAuthorizedWallet,
}
