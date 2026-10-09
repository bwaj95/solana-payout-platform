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

    #[msg("Payment id must be greater than zero")]
    InvalidPaymentId,

    #[msg("Payment amount must be greater than zero")]
    InvalidPaymentAmount,

    #[msg("Execute-after timestamp cannot be negative")]
    InvalidExecuteAfter,

    #[msg("The member must be an administrator or preparer")]
    MissingPaymentCreatorRole,

    #[msg("The recipient does not belong to this organization")]
    RecipientOrganizationMismatch,

    #[msg("The recipient is inactive")]
    InactiveRecipient,

    #[msg("The policy does not belong to this organization")]
    PolicyOrganizationMismatch,

    #[msg("The policy version is inactive")]
    InactivePolicy,

    #[msg("The member must be an administrator or approver")]
    MissingPaymentApproverRole,

    #[msg("The member does not have permission to execute payments")]
    MissingPaymentExecutorRole,

    #[msg("The member does not have permission to finalize payment approval")]
    MissingPaymentFinalizerRole,

    #[msg("The vault doesn't have enough balance to cover the payment")]
    InsufficientVaultBalance,

    #[msg("The payment terms don't match")]
    PaymentTermsMismatch,

    #[msg("The payment is not accepting approvals")]
    PaymentNotAcceptingApprovals,

    #[msg("The payment has an invalid reservation state for approval")]
    InvalidPaymentReservationState,

    #[msg("The member is not an eligible approver for this policy version")]
    ApproverNotEligibleForPolicy,

    #[msg("The payment settlement rail is not supported")]
    UnsupportedSettlementRail,

    #[msg("Remaining approval accounts must be Approval and Member pairs")]
    InvalidRemainingApprovalAccounts,

    #[msg("Too many approval witness accounts were supplied")]
    TooManyApprovalAccounts,

    #[msg("The supplied Approval account is invalid")]
    InvalidApprovalAccount,

    #[msg("The supplied Member account is invalid")]
    InvalidMemberAccount,

    #[msg("The same member cannot be counted more than once")]
    DuplicateApprovalMember,

    #[msg("The valid approval count has not reached the policy threshold")]
    ApprovalThresholdNotMet,

    #[msg("The vault reserved total exceeds its token balance")]
    VaultReservationInvariantViolation,

    #[msg("Arithmetic overflow")]
    ArithmeticOverflow,

    #[msg("The payment has an invalid approval state for execution")]
    InvalidPaymentApprovalState,

    #[msg("The payment has an invalid reservation state for execution")]
    InvalidPaymentReservationExecutionState,

    #[msg("Need to hit the execution time to make the payment")]
    ExecutionTimeNotReached,

    #[msg("The vault reserved total is insufficient to make the payment")]
    VaultReservationInsufficient,

    #[msg("The payment cannot be cancelled from its current payment and reservation states")]
    InvalidPaymentCancellationState,

    #[msg("The new recipient destination is the same as the current destination")]
    RecipientDestinationUnchanged,

    #[msg("The organization is already in the requested pause state")]
    OrganizationPauseStateUnchanged,

    #[msg("The member does not have permission to withdraw vault funds")]
    MissingVaultWithdrawalRole,

    #[msg("Vault withdrawal amount must be greater than zero")]
    InvalidVaultWithdrawalAmount,

    #[msg("The vault withdrawal destination is invalid")]
    InvalidVaultWithdrawalDestination,

    #[msg("The withdrawal amount exceeds the vault's unreserved balance")]
    VaultWithdrawalExceedsAvailableBalance,
}
