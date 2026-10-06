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

    #[msg("Policy ID must be greater than zero")]
    InvalidPolicyId,

    #[msg("Policy version must be greater than zero")]
    InvalidPolicyVersion,

    #[msg("The policy must contain at least one eligible member")]
    EmptyPolicyMembers,

    #[msg("The policy contains too many eligible members")]
    TooManyPolicyMembers,

    #[msg("The approval threshold is invalid")]
    InvalidPolicyThreshold,

    #[msg("The policy contains the same member more than once")]
    DuplicatePolicyMember,

    #[msg("An eligible member account is invalid")]
    InvalidPolicyMember,

    #[msg("An eligible policy member is inactive")]
    InactivePolicyMember,

    #[msg("An eligible policy member does not have the approver role")]
    PolicyMemberMissingApproverRole,
}
