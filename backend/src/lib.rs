use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

pub mod config;
pub mod state;

pub mod account_validation;
mod auth;
mod domain;
mod error;
pub mod jobs;
mod organization_initialization;
mod organizations;
pub mod pdas;
pub mod reconciliation;
pub mod reconciliation_worker;
pub mod solana_rpc;
pub mod transaction_validation;

pub use domain::TransactionSubmissionStatus;

pub use config::{Config, WorkerConfig};
pub use state::AppState;

#[derive(Serialize)]
struct StatusResponse {
    service: &'static str,
    status: &'static str,
}

pub fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .merge(auth::router())
        .merge(organizations::router())
        .merge(organization_initialization::router())
        .with_state(state)
}

async fn health() -> Json<StatusResponse> {
    Json(StatusResponse {
        service: "payout-backend",
        status: "ok",
    })
}

async fn ready(
    State(state): State<AppState>,
) -> Result<Json<StatusResponse>, (StatusCode, Json<StatusResponse>)> {
    let database_result = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await;

    match database_result {
        Ok(_) => Ok(Json(StatusResponse {
            service: "payout-backend",
            status: "ready",
        })),

        Err(error) => {
            eprintln!("Database readiness check failed: {error}");

            Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(StatusResponse {
                    service: "payout-backend",
                    status: "unavailable",
                }),
            ))
        }
    }
}
