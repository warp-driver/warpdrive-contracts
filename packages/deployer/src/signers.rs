//! Signer-set management against the variant's security contract:
//! `add-signer`, `remove-signer`, `set-threshold`.
//!
//! Each op works in two modes:
//! - **direct** (pre-handover): the deployer is the security contract's admin
//!   and calls it directly;
//! - **`--via project-root`** (post-handover, PLAN.md §5): the security
//!   contract's admin is now project_root, so changes route through
//!   project_root's `add_{secp,ed}_signer` / `remove_*` / `set_threshold`
//!   forwarders, authorized by project_root's admin.
//!
//! Both modes — and both schemes (secp256k1/ed25519) — are unified behind the
//! [`SecurityClient`] enum so each public fn is a thin wrapper rather than a
//! four-way `match scheme { … }` ladder.

use warpdrive_client::ed25519_security::Ed25519SecurityClient;
use warpdrive_client::project_root::ProjectRootClient;
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use wasi_soroban_rs::{
    Account, ClientContractConfigs, ContractId, SorobanHelperError, SorobanTransactionResponse,
};

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

/// Resolve which contract the call targets: the security contract directly, or
/// project_root (the forwarder) when `via_project_root`. Validates the scheme
/// against the manifest variant either way.
fn signer_target(
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    via_project_root: bool,
) -> Result<ContractId> {
    require_scheme_matches(manifest, scheme)?;
    if via_project_root {
        require_project_root(manifest)
    } else {
        require_security(manifest)
    }
}

// ── Unified client ───────────────────────────────────────────────────────────
//
// The security clients and `ProjectRootClient` are distinct concrete types
// (async-fn-in-trait → not object-safe), and each scheme is its own type. This
// enum is the single place that knows the scheme/mode → method mapping, so the
// public fns don't each carry a `match scheme { … try_into … }` ladder.

enum SecurityClient {
    Secp(Secp256k1SecurityClient),
    Ed(Ed25519SecurityClient),
    /// Forward through project_root; `scheme` selects the forwarder method.
    Proxy {
        client: ProjectRootClient,
        scheme: Scheme,
    },
}

/// `parse_key` already validated the length, so these conversions never fail.
fn to_secp(key: &[u8]) -> [u8; 33] {
    key.try_into().expect("length checked by parse_key")
}
fn to_ed(key: &[u8]) -> [u8; 32] {
    key.try_into().expect("length checked by parse_key")
}

impl SecurityClient {
    fn new(configs: ClientContractConfigs, scheme: Scheme, via_project_root: bool) -> Self {
        if via_project_root {
            SecurityClient::Proxy {
                client: ProjectRootClient::new(configs),
                scheme,
            }
        } else {
            match scheme {
                Scheme::Secp256k1 => SecurityClient::Secp(Secp256k1SecurityClient::new(configs)),
                Scheme::Ed25519 => SecurityClient::Ed(Ed25519SecurityClient::new(configs)),
            }
        }
    }

    async fn add_signer(
        &mut self,
        key: &[u8],
        weight: u64,
    ) -> std::result::Result<SorobanTransactionResponse, SorobanHelperError> {
        match self {
            SecurityClient::Secp(c) => c.add_signer(to_secp(key), weight).await,
            SecurityClient::Ed(c) => c.add_signer(to_ed(key), weight).await,
            SecurityClient::Proxy { client, scheme } => match *scheme {
                Scheme::Secp256k1 => client.add_secp256k1_signer(to_secp(key), weight).await,
                Scheme::Ed25519 => client.add_ed25519_signer(to_ed(key), weight).await,
            },
        }
    }

    async fn remove_signer(
        &mut self,
        key: &[u8],
    ) -> std::result::Result<SorobanTransactionResponse, SorobanHelperError> {
        match self {
            SecurityClient::Secp(c) => c.remove_signer(to_secp(key)).await,
            SecurityClient::Ed(c) => c.remove_signer(to_ed(key)).await,
            SecurityClient::Proxy { client, scheme } => match *scheme {
                Scheme::Secp256k1 => client.remove_secp256k1_signer(to_secp(key)).await,
                Scheme::Ed25519 => client.remove_ed25519_signer(to_ed(key)).await,
            },
        }
    }

    async fn set_threshold(
        &mut self,
        numerator: u64,
        denominator: u64,
    ) -> std::result::Result<SorobanTransactionResponse, SorobanHelperError> {
        match self {
            SecurityClient::Secp(c) => c.set_threshold(numerator, denominator).await,
            SecurityClient::Ed(c) => c.set_threshold(numerator, denominator).await,
            SecurityClient::Proxy { client, .. } => {
                client.set_threshold(numerator, denominator).await
            }
        }
    }
}

// ── Subcommands ──────────────────────────────────────────────────────────────

/// `add-signer`: register or update a signer's weight. `via_project_root`
/// selects the post-handover forwarder path.
#[allow(clippy::too_many_arguments)] // independent CLI params; bundling would obscure
pub async fn add_signer(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    weight: u64,
    via_project_root: bool,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let target = signer_target(manifest, scheme, via_project_root)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, target)?;

    let resp = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            SecurityClient::new(configs, scheme, via_project_root)
                .add_signer(&key, weight)
                .await
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `remove-signer`: drop a signer.
pub async fn remove_signer(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    key_hex: &str,
    via_project_root: bool,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let target = signer_target(manifest, scheme, via_project_root)?;
    let key = parse_key(scheme, key_hex)?;
    let configs = client_configs(net, account, target)?;

    let resp = retry(retry_cfg, || {
        let configs = configs.clone();
        let key = key.clone();
        async move {
            SecurityClient::new(configs, scheme, via_project_root)
                .remove_signer(&key)
                .await
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}

/// `set-threshold`: update `numerator/denominator`.
#[allow(clippy::too_many_arguments)] // independent CLI params; bundling would obscure
pub async fn set_threshold(
    net: &NetworkConfig,
    account: &Account,
    manifest: &StellarDeployManifest,
    scheme: Scheme,
    numerator: u64,
    denominator: u64,
    via_project_root: bool,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let target = signer_target(manifest, scheme, via_project_root)?;
    let configs = client_configs(net, account, target)?;

    let resp = retry(retry_cfg, || {
        let configs = configs.clone();
        async move {
            SecurityClient::new(configs, scheme, via_project_root)
                .set_threshold(numerator, denominator)
                .await
        }
    })
    .await?;

    Ok(tx_hash(&resp))
}
