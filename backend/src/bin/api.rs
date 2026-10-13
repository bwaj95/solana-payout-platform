use payout_backend::{AppState, Config, build_router};
use sqlx::postgres::PgPoolOptions;
use std::{error::Error, time::Duration};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let config = Config::from_env()?;

    let db = PgPoolOptions::new()
        .max_connections(config.database_max_connections)
        .acquire_timeout(Duration::from_secs(5))
        .connect(&config.database_url)
        .await?;

    let state = AppState::new(db);
    let app = build_router(state);

    let listener = TcpListener::bind(config.bind_address).await?;

    println!("API listening on http://{}", config.bind_address);

    axum::serve(listener, app).await?;

    Ok(())
}
