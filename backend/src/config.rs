use std::{
    env,
    net::{AddrParseError, SocketAddr},
    num::{NonZeroU32, ParseIntError},
    time::Duration,
};

use thiserror::Error;

/*
 * Configuration required only by the HTTP API process.
 */
#[derive(Clone)]
pub struct Config {
    pub bind_address: SocketAddr,

    pub database_url: String,
    pub database_max_connections: u32,

    pub solana_cluster: String,
    pub solana_rpc_url: String,
    pub solana_program_id: String,
}

/*
 * Configuration required only by the background worker process.
 *
 * Keeping this separate from Config means the API does not need to know
 * about polling intervals, leases or recovery batches.
 */
#[derive(Clone)]
pub struct WorkerConfig {
    pub database_url: String,
    pub database_max_connections: u32,

    pub solana_cluster: String,
    pub solana_rpc_url: String,
    pub solana_program_id: String,

    pub poll_interval: Duration,
    pub lease_seconds: i64,
    pub defer_seconds: i64,
    pub retry_delay_seconds: i64,
    pub recovery_batch_size: i64,
    pub rpc_timeout: Duration,
}

/*
 * Private intermediate configuration shared by the API and worker parsers.
 *
 * This avoids duplicating database and Solana environment parsing while
 * still exposing separate public configuration contracts to each process.
 */
struct SharedConfig {
    database_url: String,
    database_max_connections: u32,

    solana_cluster: String,
    solana_rpc_url: String,
    solana_program_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let host = env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());

        let port = env::var("API_PORT").unwrap_or_else(|_| "8080".to_owned());

        let bind_address: SocketAddr = format!("{host}:{port}").parse()?;

        let shared = load_shared_config()?;

        Ok(Self {
            bind_address,

            database_url: shared.database_url,
            database_max_connections: shared.database_max_connections,

            solana_cluster: shared.solana_cluster,
            solana_rpc_url: shared.solana_rpc_url,
            solana_program_id: shared.solana_program_id,
        })
    }
}

impl WorkerConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let shared = load_shared_config()?;

        /*
         * Poll interval controls how frequently an idle worker checks
         * PostgreSQL for due jobs.
         */
        let poll_interval_ms = positive_u32_from_env("WORKER_POLL_INTERVAL_MS", "1000")?;

        /*
         * The logical job lease must be comfortably longer than the RPC
         * calls and final database settlement.
         */
        let lease_seconds = positive_u32_from_env("WORKER_LEASE_SECONDS", "60")?;

        /*
         * A successfully processed but non-terminal Solana transaction is
         * checked again after this delay.
         */
        let defer_seconds = positive_u32_from_env("WORKER_DEFER_SECONDS", "5")?;

        /*
         * A genuine processing or RPC failure is retried after this delay.
         * We can add capped exponential backoff after the first worker flow
         * is complete.
         */
        let retry_delay_seconds = positive_u32_from_env("WORKER_RETRY_DELAY_SECONDS", "5")?;

        /*
         * Lease recovery is bounded so one cycle does not lock an
         * unbounded number of expired jobs.
         */
        let recovery_batch_size = positive_u32_from_env("WORKER_RECOVERY_BATCH_SIZE", "100")?;

        let rpc_timeout_seconds = positive_u32_from_env("SOLANA_RPC_TIMEOUT_SECONDS", "10")?;

        Ok(Self {
            database_url: shared.database_url,
            database_max_connections: shared.database_max_connections,

            solana_cluster: shared.solana_cluster,
            solana_rpc_url: shared.solana_rpc_url,
            solana_program_id: shared.solana_program_id,

            poll_interval: Duration::from_millis(u64::from(poll_interval_ms)),
            lease_seconds: i64::from(lease_seconds),
            defer_seconds: i64::from(defer_seconds),
            retry_delay_seconds: i64::from(retry_delay_seconds),
            recovery_batch_size: i64::from(recovery_batch_size),
            rpc_timeout: Duration::from_secs(u64::from(rpc_timeout_seconds)),
        })
    }
}

/*
 * Load infrastructure settings required by both binaries.
 *
 * This function deliberately does not read API-only or worker-only
 * variables.
 */
fn load_shared_config() -> Result<SharedConfig, ConfigError> {
    let database_url = env::var("DATABASE_URL").map_err(|_| ConfigError::MissingDatabaseUrl)?;

    let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
        .unwrap_or_else(|_| "10".to_owned())
        .parse::<NonZeroU32>()
        .map_err(ConfigError::InvalidDatabaseMaxConnections)?
        .get();

    let solana_cluster = env::var("SOLANA_CLUSTER").unwrap_or_else(|_| "devnet".to_owned());

    if !matches!(
        solana_cluster.as_str(),
        "localnet" | "devnet" | "testnet" | "mainnet-beta"
    ) {
        return Err(ConfigError::InvalidSolanaCluster(solana_cluster));
    }

    let solana_rpc_url =
        env::var("SOLANA_RPC_URL").map_err(|_| ConfigError::MissingSolanaRpcUrl)?;

    if solana_rpc_url.trim().is_empty() {
        return Err(ConfigError::MissingSolanaRpcUrl);
    }

    let solana_program_id =
        env::var("SOLANA_PROGRAM_ID").map_err(|_| ConfigError::MissingSolanaProgramId)?;

    validate_solana_address(&solana_program_id)?;

    Ok(SharedConfig {
        database_url,
        database_max_connections,

        solana_cluster,
        solana_rpc_url,
        solana_program_id,
    })
}

/*
 * Parse a positive u32 environment variable with a default value.
 *
 * NonZeroU32 rejects both zero and negative values. Restricting these
 * values during startup is safer than discovering an invalid delay or
 * batch size while the worker is already running.
 */
fn positive_u32_from_env(
    variable: &'static str,
    default_value: &'static str,
) -> Result<u32, ConfigError> {
    env::var(variable)
        .unwrap_or_else(|_| default_value.to_owned())
        .parse::<NonZeroU32>()
        .map(NonZeroU32::get)
        .map_err(|source| ConfigError::InvalidPositiveInteger { variable, source })
}

fn validate_solana_address(address: &str) -> Result<(), ConfigError> {
    let bytes = bs58::decode(address)
        .into_vec()
        .map_err(|_| ConfigError::InvalidSolanaProgramId)?;

    if bytes.len() != 32 {
        return Err(ConfigError::InvalidSolanaProgramId);
    }

    if bs58::encode(bytes).into_string() != address {
        return Err(ConfigError::InvalidSolanaProgramId);
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("API_HOST and API_PORT do not form a valid socket address")]
    InvalidBindAddress(#[from] AddrParseError),

    #[error("DATABASE_URL is not set")]
    MissingDatabaseUrl,

    #[error("DATABASE_MAX_CONNECTIONS must be a positive u32")]
    InvalidDatabaseMaxConnections(#[source] ParseIntError),

    #[error("SOLANA_CLUSTER must be localnet, devnet, testnet or mainnet-beta; received {0}")]
    InvalidSolanaCluster(String),

    #[error("SOLANA_RPC_URL is not set")]
    MissingSolanaRpcUrl,

    #[error("SOLANA_PROGRAM_ID is not set")]
    MissingSolanaProgramId,

    #[error("SOLANA_PROGRAM_ID is not a valid Solana public key")]
    InvalidSolanaProgramId,

    #[error("{variable} must be a positive u32")]
    InvalidPositiveInteger {
        variable: &'static str,

        #[source]
        source: ParseIntError,
    },
}
