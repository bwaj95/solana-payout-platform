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
}

impl Config {
    pub fn from_env() -> Result<Self, AddrParseError> {
        dotenvy::dotenv().ok();

        let host = env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_owned());
        let port = env::var("API_PORT").unwrap_or_else(|_| "8080".to_owned());

        let bind_address: SocketAddr = format!("{host}:{port}").parse()?;

        let database_url: String = env::var("DATABASE_URL")
            .map_err(|_| ConfigError::MissingDatabaseUrl)
            .unwrap();
        let database_max_connections = env::var("DATABASE_MAX_CONNECTIONS")
            .unwrap_or_else(|_| "10".to_owned())
            .parse::<NonZeroU32>()
            .map_err(ConfigError::InvalidDatabaseMaxConnections)
            .unwrap()
            .get();

        Ok(Self {
            bind_address,
            database_url,
            database_max_connections,
        })
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("API_HOST and API_PORT do not form a valid socket address")]
    InvalidBindAddress(#[from] AddrParseError),

    #[error("DATABASE_URL is not set")]
    MissingDatabaseUrl,

    #[error("DATABASE_MAX_CONNECTIONS must be a positive u32")]
    InvalidDatabaseMaxConnections(#[source] ParseIntError),
}
