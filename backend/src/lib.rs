use axum::{Json, Router, extract::State, http::StatusCode, routing::get};
use serde::Serialize;

pub mod config;
pub mod state;

mod auth;
mod domain;
mod error;
mod organizations;

pub use config::Config;
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
