//! Error type for the deployer. Wraps the underlying soroban/io/json/hex
//! errors and adds a few domain-specific variants.

use thiserror::Error;
use wasi_soroban_rs::SorobanHelperError;

use crate::retry::Retryable;

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

impl Retryable for SorobanHelperError {
    fn is_retryable(&self) -> bool {
        // `NotSupported` covers "Address authorization not yet supported": the
        // signer can't satisfy an Address-credential auth requirement, and a
        // re-simulation yields the identical result. Treat it (and any other
        // NotSupported) as permanent. Everything else (RPC/network/simulation
        // hiccups) may be transient, so stays retryable.
        !matches!(self, SorobanHelperError::NotSupported(_))
    }
}

impl Retryable for DeployerError {
    fn is_retryable(&self) -> bool {
        match self {
            DeployerError::Soroban(e) => e.is_retryable(),
            // Transient: friendbot / network HTTP.
            DeployerError::Http(_) => true,
            // Permanent: re-running can't change the outcome.
            DeployerError::InvalidArgument(_)
            | DeployerError::Manifest(_)
            | DeployerError::Config(_)
            | DeployerError::Identity(_)
            | DeployerError::Hex(_)
            | DeployerError::Json(_)
            | DeployerError::Io(_) => false,
        }
    }
}
