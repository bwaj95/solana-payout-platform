use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("database operation failed")]
    Database(#[from] sqlx::Error),

    #[error("organization name must contain between 1 and 120 characters")]
    InvalidOrganizationName,

    #[error("organization not found")]
    OrganizationNotFound,

    #[error("wallet address is not a valid Solana public key")]
    InvalidWalletAddress,

    #[error("authentication challenge was not found")]
    AuthenticationChallengeNotFound,

    #[error("authentication challenge has expired")]
    AuthenticationChallengeExpired,

    #[error("authentication challenge has already been consumed")]
    AuthenticationChallengeConsumed,

    #[error("authentication wallet does not match the challenge wallet")]
    AuthenticationWalletMismatch,

    #[error("wallet signature is invalid")]
    InvalidWalletSignature,

    #[error("authentication token is missing")]
    MissingAuthenticationToken,

    #[error("authentication token is invalid")]
    InvalidAuthenticationToken,

    #[error("member display name must contain between 1 and 120 characters")]
    InvalidMemberDisplayName,

    #[error("member external reference must contain between 1 and 120 characters")]
    InvalidMemberExternalReference,

    #[error("organization owner was not found")]
    OrganizationOwnerNotFound,

    #[error("authenticated wallet is not the organization owner")]
    OrganizationAccessDenied,

    #[error("organization cannot currently be initialized")]
    OrganizationInitializationNotAllowed,

    #[error("transaction signature is invalid")]
    InvalidTransactionSignature,

    #[error("last valid block height is invalid")]
    InvalidLastValidBlockHeight,

    #[error("idempotency key was already used for another request")]
    IdempotencyKeyConflict,

    #[error("transaction signature was already recorded")]
    TransactionSignatureAlreadyRecorded,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorBody,
}

#[derive(Serialize)]
struct ErrorBody {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message) = match &self {
            Self::Database(error) => {
                eprintln!("Database operation failed: {error}");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database_error",
                    "A database operation failed.",
                )
            }

            Self::InvalidOrganizationName => (
                StatusCode::BAD_REQUEST,
                "invalid_organization_name",
                "Organization name must contain between 1 and 120 characters.",
            ),

            Self::OrganizationNotFound => (
                StatusCode::NOT_FOUND,
                "organization_not_found",
                "Organization not found.",
            ),

            Self::InvalidWalletAddress => (
                StatusCode::BAD_REQUEST,
                "invalid_wallet_address",
                "Wallet address is not a valid Solana public key.",
            ),

            Self::AuthenticationChallengeNotFound => (
                StatusCode::NOT_FOUND,
                "authentication_challenge_not_found",
                "Authentication challenge was not found.",
            ),

            Self::AuthenticationChallengeExpired => (
                StatusCode::UNAUTHORIZED,
                "authentication_challenge_expired",
                "Authentication challenge has expired.",
            ),

            Self::AuthenticationChallengeConsumed => (
                StatusCode::CONFLICT,
                "authentication_challenge_consumed",
                "Authentication challenge has already been consumed.",
            ),

            Self::AuthenticationWalletMismatch => (
                StatusCode::UNAUTHORIZED,
                "authentication_wallet_mismatch",
                "Wallet does not match the authentication challenge.",
            ),

            Self::InvalidWalletSignature => (
                StatusCode::UNAUTHORIZED,
                "invalid_wallet_signature",
                "Wallet signature is invalid.",
            ),

            Self::MissingAuthenticationToken => (
                StatusCode::UNAUTHORIZED,
                "missing_authentication_token",
                "A bearer authentication token is required.",
            ),

            Self::InvalidAuthenticationToken => (
                StatusCode::UNAUTHORIZED,
                "invalid_authentication_token",
                "Authentication token is invalid, expired or revoked.",
            ),

            Self::InvalidMemberDisplayName => (
                StatusCode::BAD_REQUEST,
                "invalid_member_display_name",
                "Member display name must contain between 1 and 120 characters.",
            ),

            Self::InvalidMemberExternalReference => (
                StatusCode::BAD_REQUEST,
                "invalid_member_external_reference",
                "Member external reference must contain between 1 and 120 characters.",
            ),

            Self::OrganizationOwnerNotFound => (
                StatusCode::CONFLICT,
                "organization_owner_not_found",
                "The organization does not have an owner member.",
            ),

            Self::OrganizationAccessDenied => (
                StatusCode::FORBIDDEN,
                "organization_access_denied",
                "The authenticated wallet is not allowed to initialize this organization.",
            ),

            Self::OrganizationInitializationNotAllowed => (
                StatusCode::CONFLICT,
                "organization_initialization_not_allowed",
                "The organization and owner member must both be in draft status.",
            ),

            Self::InvalidTransactionSignature => (
                StatusCode::BAD_REQUEST,
                "invalid_transaction_signature",
                "Transaction signature must be a valid Solana signature.",
            ),

            Self::InvalidLastValidBlockHeight => (
                StatusCode::BAD_REQUEST,
                "invalid_last_valid_block_height",
                "Last valid block height must be nonnegative.",
            ),

            Self::IdempotencyKeyConflict => (
                StatusCode::CONFLICT,
                "idempotency_key_conflict",
                "The idempotency key was already used for a different request.",
            ),

            Self::TransactionSignatureAlreadyRecorded => (
                StatusCode::CONFLICT,
                "transaction_signature_already_recorded",
                "The transaction signature was already recorded by another submission.",
            ),
        };

        (
            status,
            Json(ErrorResponse {
                error: ErrorBody { code, message },
            }),
        )
            .into_response()
    }
}
