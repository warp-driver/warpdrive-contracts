//! Governance handover & proxy-call subcommands. Wraps the `#48` proxy methods
//! already on the clients (no client changes): every downstream + project_root
//! impls `WarpdriveClient` (`propose_admin`/`accept_admin`/`admin`/
//! `pending_admin`), and `ProjectRootClient` adds the forwarders
//! (`accept_contract_admin`, signer forwarders).
//!
//! Behavioural spec: `contracts/project-root/src/tests/governance.rs::
//! run_deployment_script`. Handlers are omitted (docker parity, PLAN.md §2), so
//! the handover loop covers `[security, verification]` only.

use std::str::FromStr;

use warpdrive_client::ed25519_security::Ed25519SecurityClient;
use warpdrive_client::ed25519_verification::Ed25519VerificationClient;
use warpdrive_client::ethereum_handler::EthereumHandlerClient;
use warpdrive_client::project_root::ProjectRootClient;
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::secp256k1_verification::Secp256k1VerificationClient;
use warpdrive_client::stellar_handler::StellarHandlerClient;
use warpdrive_client::warpdrive::WarpdriveClient;
use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress};
use wasi_soroban_rs::{Account, ContractId, Env};

use crate::config::client_configs;
use crate::error::{DeployerError, Result};
use crate::manifest::{
    StellarDeployManifest, Variant, require_handler, require_project_root, require_security,
    require_verification,
};
use crate::retry::{RetryConfig, retry};
use crate::tx_hash;

/// A governed contract addressable by the handover subcommands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Target {
    Security,
    Verification,
    Handler,
    ProjectRoot,
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Target::Security => f.write_str("security"),
            Target::Verification => f.write_str("verification"),
            Target::Handler => f.write_str("handler"),
            Target::ProjectRoot => f.write_str("project-root"),
        }
    }
}

/// Resolve the contract ID for `target` from the manifest.
fn target_id(m: &StellarDeployManifest, target: Target) -> Result<ContractId> {
    match target {
        Target::Security => require_security(m),
        Target::Verification => require_verification(m),
        Target::Handler => require_handler(m),
        Target::ProjectRoot => require_project_root(m),
    }
}

/// `ScAddress` for a contract ID (for admin comparisons).
fn contract_scaddress(id: ContractId) -> ScAddress {
    ScAddress::Contract(XdrContractId(Hash(id.0)))
}

// ── Concrete-client dispatch ─────────────────────────────────────────────────
//
// `WarpdriveClient` has `async fn`s, so it isn't dyn-compatible; we wrap the
// concrete clients in an enum and dispatch with a macro.

enum AnyClient {
    SecpSecurity(Secp256k1SecurityClient),
    EdSecurity(Ed25519SecurityClient),
    SecpVerification(Secp256k1VerificationClient),
    EdVerification(Ed25519VerificationClient),
    EthHandler(EthereumHandlerClient),
    XlmHandler(StellarHandlerClient),
    ProjectRoot(ProjectRootClient),
}

macro_rules! dispatch {
    ($self:expr, $c:ident => $body:expr) => {
        match $self {
            AnyClient::SecpSecurity($c) => $body,
            AnyClient::EdSecurity($c) => $body,
            AnyClient::SecpVerification($c) => $body,
            AnyClient::EdVerification($c) => $body,
            AnyClient::EthHandler($c) => $body,
            AnyClient::XlmHandler($c) => $body,
            AnyClient::ProjectRoot($c) => $body,
        }
    };
}

impl AnyClient {
    fn build(
        env: &Env,
        account: &Account,
        m: &StellarDeployManifest,
        target: Target,
    ) -> Result<Self> {
        let cfg = client_configs(env, account, target_id(m, target)?);
        Ok(match (target, m.variant) {
            (Target::Security, Variant::Ethereum) => {
                AnyClient::SecpSecurity(Secp256k1SecurityClient::new(cfg))
            }
            (Target::Security, Variant::Stellar) => {
                AnyClient::EdSecurity(Ed25519SecurityClient::new(cfg))
            }
            (Target::Verification, Variant::Ethereum) => {
                AnyClient::SecpVerification(Secp256k1VerificationClient::new(cfg))
            }
            (Target::Verification, Variant::Stellar) => {
                AnyClient::EdVerification(Ed25519VerificationClient::new(cfg))
            }
            (Target::Handler, Variant::Ethereum) => {
                AnyClient::EthHandler(EthereumHandlerClient::new(cfg))
            }
            (Target::Handler, Variant::Stellar) => {
                AnyClient::XlmHandler(StellarHandlerClient::new(cfg))
            }
            (Target::ProjectRoot, _) => AnyClient::ProjectRoot(ProjectRootClient::new(cfg)),
        })
    }

    async fn propose_admin(&mut self, new_admin: &str) -> Result<()> {
        dispatch!(self, c => c.propose_admin(new_admin).await).map_err(DeployerError::from)
    }

    async fn accept_admin(&mut self) -> Result<()> {
        dispatch!(self, c => c.accept_admin().await).map_err(DeployerError::from)
    }

    async fn admin(&self) -> Result<ScAddress> {
        dispatch!(self, c => c.admin().await).map_err(DeployerError::from)
    }

    async fn pending_admin(&self) -> Result<Option<ScAddress>> {
        dispatch!(self, c => c.pending_admin().await).map_err(DeployerError::from)
    }
}

// ── Subcommands ──────────────────────────────────────────────────────────────

/// `propose-admin`: start rotating `target`'s admin to `new_admin`. Signed by
/// `target`'s current admin (the deployer pre-handover).
pub async fn propose_admin(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    target: Target,
    new_admin: &str,
    retry_cfg: RetryConfig,
) -> Result<()> {
    // Validate the address up front so a typo fails before any network call.
    ScAddress::from_str(new_admin)
        .map_err(|e| DeployerError::InvalidArgument(format!("invalid --new-admin address: {e}")))?;
    retry(retry_cfg, || async move {
        AnyClient::build(env, account, m, target)?
            .propose_admin(new_admin)
            .await
    })
    .await
}

/// `accept-admin`: finish a handover, signed by whoever is `target`'s pending
/// admin (typically the owner with their own `--secret`).
pub async fn accept_admin(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    target: Target,
    retry_cfg: RetryConfig,
) -> Result<()> {
    retry(retry_cfg, || async move {
        AnyClient::build(env, account, m, target)?
            .accept_admin()
            .await
    })
    .await
}

/// `accept-contract-admin`: project_root accepts the admin of a downstream
/// contract on its own behalf. Signed by project_root's current admin.
pub async fn accept_contract_admin(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    target: Target,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let downstream = match target {
        Target::Security | Target::Verification | Target::Handler => target_id(m, target)?,
        Target::ProjectRoot => {
            return Err(DeployerError::InvalidArgument(
                "accept-contract-admin target must be security, verification or handler"
                    .to_string(),
            ));
        }
    };
    let project_root = require_project_root(m)?;
    let resp = retry(retry_cfg, || async move {
        let cfg = client_configs(env, account, project_root);
        ProjectRootClient::new(cfg)
            .accept_contract_admin(downstream)
            .await
            .map_err(DeployerError::from)
    })
    .await?;
    Ok(tx_hash(&resp))
}

/// Step 4 of a deployment: rotate the security and verification contracts'
/// admin to project_root, so project_root owns the whole pipeline. Called at
/// the tail of `deploy::deploy_pipeline` — every deployed contract ends up
/// owned by project_root. Idempotent via `admin()`/`pending_admin()` reads
/// (a re-run skips what's already adopted). Signed by the deployer, who is the
/// current admin of each downstream and of project_root.
pub async fn adopt_downstreams(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    retry_cfg: RetryConfig,
) -> Result<()> {
    let project_root = require_project_root(m)?;
    let pr_addr = contract_scaddress(project_root);
    let project_root_str = project_root.to_string();

    for target in [Target::Security, Target::Verification] {
        let current = AnyClient::build(env, account, m, target)?.admin().await?;
        if current == pr_addr {
            eprintln!("=== {target} already owned by project_root ===");
            continue;
        }
        let pending = AnyClient::build(env, account, m, target)?
            .pending_admin()
            .await?;
        if pending.as_ref() != Some(&pr_addr) {
            eprintln!("=== proposing project_root as admin of {target} ===");
            propose_admin(env, account, m, target, &project_root_str, retry_cfg).await?;
        }
        eprintln!("=== project_root accepting admin of {target} ===");
        accept_contract_admin(env, account, m, target, retry_cfg).await?;
    }
    Ok(())
}

/// `handover`: rotate project_root's own admin to `owner` (step 5). The
/// downstream contracts are already owned by project_root — that adoption
/// (step 4) happens during `deploy_pipeline`, not here. Idempotent via
/// `admin()`/`pending_admin()` reads. The owner-side `accept-admin` is left to
/// the owner.
pub async fn handover(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    owner: &str,
    retry_cfg: RetryConfig,
) -> Result<()> {
    let owner_addr = ScAddress::from_str(owner)
        .map_err(|e| DeployerError::InvalidArgument(format!("invalid --owner address: {e}")))?;

    let pr_admin = AnyClient::build(env, account, m, Target::ProjectRoot)?
        .admin()
        .await?;
    if pr_admin == owner_addr {
        eprintln!("=== project_root already owned by {owner} ===");
        return Ok(());
    }
    let pr_pending = AnyClient::build(env, account, m, Target::ProjectRoot)?
        .pending_admin()
        .await?;
    if pr_pending.as_ref() == Some(&owner_addr) {
        eprintln!("=== project_root admin already proposed to {owner}; awaiting owner accept ===");
        return Ok(());
    }
    eprintln!("=== proposing {owner} as admin of project_root ===");
    propose_admin(env, account, m, Target::ProjectRoot, owner, retry_cfg).await?;
    eprintln!("handover proposed; owner must now run `accept-admin --target project-root`");
    Ok(())
}

/// `register-handler`: track the deployed handler in project_root's handler set
/// by calling project_root's `register_handler`, signed by project_root's admin.
/// The canonical flow deploys the handler with project_root already as its admin
/// (`deploy-handler`), so this is a single call — no propose/accept dance.
/// Idempotent on-chain (re-registering an already-tracked handler is a no-op).
///
/// (A handler deployed under a different admin can instead be brought in via the
/// handover dance — `propose-admin --target handler` then
/// `accept-contract-admin --target handler` — which auto-registers on accept.)
pub async fn register_handler(
    env: &Env,
    account: &Account,
    m: &StellarDeployManifest,
    retry_cfg: RetryConfig,
) -> Result<String> {
    let project_root = require_project_root(m)?;
    let handler = require_handler(m)?;

    let resp = retry(retry_cfg, || async move {
        let cfg = client_configs(env, account, project_root);
        ProjectRootClient::new(cfg)
            .register_handler(handler)
            .await
            .map_err(DeployerError::from)
    })
    .await?;
    Ok(tx_hash(&resp))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ETHEREUM_GOLDEN: &str = include_str!("../tests/golden/deploy-ethereum.json");
    const STELLAR_GOLDEN: &str = include_str!("../tests/golden/deploy-stellar.json");

    fn eth() -> StellarDeployManifest {
        serde_json::from_str(ETHEREUM_GOLDEN).unwrap()
    }
    fn xlm() -> StellarDeployManifest {
        serde_json::from_str(STELLAR_GOLDEN).unwrap()
    }

    #[test]
    fn target_id_resolves_per_variant() {
        let e = eth();
        assert_eq!(
            target_id(&e, Target::Security).unwrap(),
            e.security().unwrap()
        );
        assert_eq!(
            target_id(&e, Target::Verification).unwrap(),
            e.verification().unwrap()
        );
        assert_eq!(
            target_id(&e, Target::ProjectRoot).unwrap(),
            e.project_root().unwrap()
        );

        let s = xlm();
        assert_eq!(
            target_id(&s, Target::Security).unwrap(),
            s.contracts.ed25519_security.unwrap()
        );
    }

    #[test]
    fn missing_contract_is_an_error() {
        let mut m = StellarDeployManifest::new("GABC".to_string(), Variant::Ethereum);
        assert!(target_id(&m, Target::Security).is_err());
        m.contracts.secp256k1_security = Some(ContractId([1; 32]));
        assert!(target_id(&m, Target::Security).is_ok());
        assert!(target_id(&m, Target::Verification).is_err());
    }

    #[test]
    fn contract_scaddress_matches_xdr_form() {
        let id = ContractId([5; 32]);
        assert_eq!(
            contract_scaddress(id),
            ScAddress::Contract(XdrContractId(Hash([5; 32])))
        );
    }
}
