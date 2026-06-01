//! Error type for the deployer. Wraps the underlying soroban/io/json/hex
//! errors and adds a few domain-specific variants.

use thiserror::Error;
use wasi_soroban_rs::SorobanHelperError;

#[derive(Debug, Error)]
pub enum DeployerError {
    #[error("soroban error: {0}")]
    Soroban(#[from] SorobanHelperError),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid hex: {0}")]
    Hex(#[from] hex::FromHexError),

    /// A required configuration value was missing or malformed.
    #[error("configuration error: {0}")]
    Config(String),

    /// Could not resolve / parse a BYOK identity.
    #[error("identity error: {0}")]
    Identity(String),

    /// The manifest was missing a contract, or the requested scheme did not
    /// match the manifest's variant.
    #[error("manifest error: {0}")]
    Manifest(String),

    /// An argument failed validation (e.g. wrong key length for a scheme).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    /// A friendbot / network HTTP request failed.
    #[error("http error: {0}")]
    Http(String),
}

pub type Result<T> = std::result::Result<T, DeployerError>;
