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
