//! Direct signer-set management against the variant's security contract:
//! `add-signer`, `remove-signer`, `set-threshold`. These are the pre-handover
//! (deployer-is-admin) operations that match the shell deployer 1:1.

use warpdrive_client::ed25519_security::Ed25519SecurityClient;
use warpdrive_client::project_root::ProjectRootClient;
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use wasi_soroban_rs::{Account, SorobanTransactionResponse};

use crate::config::{NetworkConfig, client_configs};
use crate::error::{DeployerError, Result};
use crate::manifest::{StellarDeployManifest, Variant, require_project_root, require_security};
use crate::retry::{RetryConfig, retry};
use crate::tx_hash;

/// Signature scheme of a security contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Scheme {
    Secp256k1,
    Ed25519,
}

impl Scheme {
    /// Expected public-key length in bytes.
    pub fn key_len(self) -> usize {
        match self {
            Scheme::Secp256k1 => 33,
            Scheme::Ed25519 => 32,
        }
    }

    /// The manifest variant this scheme belongs to.
    pub fn variant(self) -> Variant {
        match self {
            Scheme::Secp256k1 => Variant::Ethereum,
            Scheme::Ed25519 => Variant::Stellar,
        }
    }
}

impl std::fmt::Display for Scheme {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Scheme::Secp256k1 => f.write_str("secp256k1"),
            Scheme::Ed25519 => f.write_str("ed25519"),
        }
    }
}

/// Parse a hex public key (`0x`-prefix optional) and validate its length for
/// `scheme`. The shell silently passed through bad bytes; we reject them.
pub fn parse_key(scheme: Scheme, key_hex: &str) -> Result<Vec<u8>> {
    let trimmed = key_hex.trim();
    let cleaned = trimmed.strip_prefix("0x").unwrap_or(trimmed);
    let bytes = hex::decode(cleaned)?;
    if bytes.len() != scheme.key_len() {
        return Err(DeployerError::InvalidArgument(format!(
            "{scheme} key must be {} bytes ({} hex chars), got {}",
            scheme.key_len(),
            scheme.key_len() * 2,
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Ensure `scheme` agrees with the manifest's variant. A single-variant
/// manifest pins exactly one security contract, so `secp256k1` requires an
/// `ethereum` manifest and `ed25519` requires a `stellar` one.
fn require_scheme_matches(manifest: &StellarDeployManifest, scheme: Scheme) -> Result<()> {
    if scheme.variant() != manifest.variant {
        return Err(DeployerError::Manifest(format!(
            "manifest is a `{}` deploy; --scheme {scheme} requires a `{}` manifest",
            manifest.variant,
            scheme.variant()
        )));
    }
    Ok(())
}

/// Resolve the security contract for `scheme`, ensuring the manifest's variant
/// matches.
fn security_for(
    manifest: &StellarDeployManifest,
    scheme: Scheme,
) -> Result<wasi_soroban_rs::ContractId> {
    require_scheme_matches(manifest, scheme)?;
    require_security(manifest)
}

/// `add-signer`: register or update a signer's weight on the security contract.
pub async fn add_signer(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    weight: u64,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let security = security_for(manifest, scheme)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, security)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            match scheme {
                Scheme::Secp256k1 => {
                    let arr: [u8; 33] = key.try_into().expect("length checked by parse_key");
                    Secp256k1SecurityClient::new(configs)
                        .add_signer(arr, weight)
                        .await
                }
                Scheme::Ed25519 => {
                    let arr: [u8; 32] = key.try_into().expect("length checked by parse_key");
                    Ed25519SecurityClient::new(configs)
                        .add_signer(arr, weight)
                        .await
                }
            }
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `remove-signer`: drop a signer from the security contract.
pub async fn remove_signer(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let security = security_for(manifest, scheme)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, security)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            match scheme {
                Scheme::Secp256k1 => {
                    let arr: [u8; 33] = key.try_into().expect("length checked by parse_key");
                    Secp256k1SecurityClient::new(configs)
                        .remove_signer(arr)
                        .await
                }
                Scheme::Ed25519 => {
                    let arr: [u8; 32] = key.try_into().expect("length checked by parse_key");
                    Ed25519SecurityClient::new(configs).remove_signer(arr).await
                }
            }
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `set-threshold`: update `numerator/denominator` on the security contract.
pub async fn set_threshold(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    numerator: u64,
    denominator: u64,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let security = security_for(manifest, scheme)?;
    let configs = client_configs(net, account, security)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        async move {
            match scheme {
                Scheme::Secp256k1 => {
                    Secp256k1SecurityClient::new(configs)
                        .set_threshold(numerator, denominator)
                        .await
                }
                Scheme::Ed25519 => {
                    Ed25519SecurityClient::new(configs)
                        .set_threshold(numerator, denominator)
                        .await
                }
            }
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

// ── Proxy mode: route through project_root's forwarders ──────────────────────
//
// After the admin handover (PLAN.md §5), the deployer/owner can no longer call
// the security contract directly; signer changes route through project_root's
// `add_{secp,ed}_signer` / `remove_*` / `set_threshold` forwarders, authorized
// by project_root's admin.

/// `add-signer --via project-root`.
pub async fn add_signer_via_project_root(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    weight: u64,
    retry_cfg: RetryConfig,
) -> Result<String> {
    require_scheme_matches(manifest, scheme)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, require_project_root(manifest)?)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            match scheme {
                Scheme::Secp256k1 => {
                    let arr: [u8; 33] = key.try_into().expect("length checked by parse_key");
                    ProjectRootClient::new(configs)
                        .add_secp256k1_signer(arr, weight)
                        .await
                }
                Scheme::Ed25519 => {
                    let arr: [u8; 32] = key.try_into().expect("length checked by parse_key");
                    ProjectRootClient::new(configs)
                        .add_ed25519_signer(arr, weight)
                        .await
                }
            }
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `remove-signer --via project-root`.
pub async fn remove_signer_via_project_root(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    retry_cfg: RetryConfig,
) -> Result<String> {
    require_scheme_matches(manifest, scheme)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, require_project_root(manifest)?)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            match scheme {
                Scheme::Secp256k1 => {
                    let arr: [u8; 33] = key.try_into().expect("length checked by parse_key");
                    ProjectRootClient::new(configs)
                        .remove_secp256k1_signer(arr)
                        .await
                }
                Scheme::Ed25519 => {
                    let arr: [u8; 32] = key.try_into().expect("length checked by parse_key");
                    ProjectRootClient::new(configs)
                        .remove_ed25519_signer(arr)
                        .await
                }
            }
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `set-threshold --via project-root`.
pub async fn set_threshold_via_project_root(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    numerator: u64,
    denominator: u64,
    retry_cfg: RetryConfig,
) -> Result<String> {
    require_scheme_matches(manifest, scheme)?;
    let configs = client_configs(net, account, require_project_root(manifest)?)?;

    let resp: SorobanTransactionResponse = retry(retry_cfg, || {
        let configs = configs.clone();
        async move {
            ProjectRootClient::new(configs)
                .set_threshold(numerator, denominator)
                .await
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}
