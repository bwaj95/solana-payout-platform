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

    #[msg("Vault id must be greater than zero")]
    InvalidVaultId,

    #[msg("The organization is paused")]
    OrganizationPaused,

    #[msg("The member must be an admin or preparer")]
    MissingRecipientRegistrarRole,

    #[msg("Recipient id must be greater than zero")]
    InvalidRecipientId,

    #[msg("Recipient destination cannot be the default public key")]
    InvalidRecipientDestination,

    #[msg("The vault does not belong to this organization")]
    VaultOrganizationMismatch,

    #[msg("The vault is inactive")]
    InactiveVault,

    #[msg("The vault does not support SPL token recipients")]
    UnsupportedVaultAsset,

    #[msg("The supplied mint does not match the vault mint")]
    VaultMintMismatch,

    #[msg("The destination token account has the wrong mint")]
    RecipientTokenMintMismatch,

    #[msg("The destination token account has the wrong authority")]
    RecipientTokenOwnerMismatch,

    #[msg("The supplied token account is not the canonical destination ATA")]
    InvalidRecipientTokenAccount,
}
