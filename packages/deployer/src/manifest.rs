//! Thin wrapper over the shared `StellarDeployManifest` (defined in
//! `warpdrive-client::loader` behind its `manifest` feature) plus a few
//! "require this contract is present" helpers.

use std::path::Path;

pub use warpdrive_client::loader::{ManifestContracts, StellarDeployManifest, Variant};
use wasi_soroban_rs::ContractId;

use crate::error::{DeployerError, Result};

/// Load a manifest from `path` (error if missing or malformed).
pub fn load(path: &Path) -> Result<StellarDeployManifest> {
    StellarDeployManifest::load(path).map_err(DeployerError::from)
}

/// The project_root contract ID, erroring if the manifest doesn't have it yet.
pub fn require_project_root(m: &StellarDeployManifest) -> Result<ContractId> {
    m.project_root().ok_or_else(|| {
        DeployerError::Manifest("project_root contract not present in manifest".to_string())
    })
}

/// The variant's security contract ID, erroring if not present.
pub fn require_security(m: &StellarDeployManifest) -> Result<ContractId> {
    m.security().ok_or_else(|| {
        DeployerError::Manifest(format!(
            "{} security contract not present in manifest",
            m.variant
        ))
    })
}
