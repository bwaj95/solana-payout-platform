use std::{
    env,
    net::{AddrParseError, SocketAddr},
    num::{NonZeroU32, ParseIntError},
};

use thiserror::Error;

#[derive(Clone)]
pub struct Config {
    pub bind_address: SocketAddr,
    pub database_url: String,
    pub database_max_connections: u32,

    pub solana_cluster: String,
    pub solana_rpc_url: String,
    pub solana_program_id: String,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        dotenvy::dotenv().ok();

        let host = env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());

        let port = env::var("API_PORT").unwrap_or_else(|_| "8080".to_owned());

        let bind_address: SocketAddr = format!("{host}:{port}").parse()?;

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

        Ok(Self {
            bind_address,
            database_url,
            database_max_connections,

            solana_cluster,
            solana_rpc_url,
            solana_program_id,
        })
    }
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
}
