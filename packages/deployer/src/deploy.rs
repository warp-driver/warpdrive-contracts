//! Idempotent deploy pipeline. Mirrors `deploy.sh`: deploys the variant's
//! security + verification contracts and project_root (no handlers — docker
//! parity, PLAN.md §2), checkpointing the manifest after each step so a
//! mid-run abort + re-run resumes exactly where it stopped.

use std::path::{Path, PathBuf};

use warpdrive_client::project_root::VerificationType;
use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress, ScVal};
use wasi_soroban_rs::{Account, Contract, ContractId, Env, IntoScVal};

use crate::config::NetworkConfig;
use crate::error::{DeployerError, Result};
use crate::manifest::{StellarDeployManifest, Variant};
use crate::retry::{RetryConfig, retry};

// Wasm filenames, resolved against `--wasm-dir`.
const SECP_SECURITY_WASM: &str = "warpdrive_secp256k1_security.wasm";
const SECP_VERIFICATION_WASM: &str = "warpdrive_secp256k1_verification.wasm";
const ED_SECURITY_WASM: &str = "warpdrive_ed25519_security.wasm";
const ED_VERIFICATION_WASM: &str = "warpdrive_ed25519_verification.wasm";
const PROJECT_ROOT_WASM: &str = "warpdrive_project_root.wasm";

pub const DEFAULT_PROJECT_SPEC_REPO: &str = "ipfs://REPLACE_ME";
pub const DEFAULT_THRESHOLD: (u64, u64) = (2, 3);

/// Inputs to a single-pipeline deploy.
#[derive(Clone, Debug)]
pub struct DeployParams {
    pub variant: Variant,
    pub wasm_dir: PathBuf,
    pub project_spec_repo: String,
    /// `(numerator, denominator)` for the security contract's threshold.
    pub threshold: (u64, u64),
    pub verification_type: VerificationType,
}

// ── Constructor-arg builders (pure; unit-tested in tests/encoding.rs) ────────

/// `ScVal::Address` for a contract ID.
pub fn contract_scval(id: ContractId) -> ScVal {
    ScVal::Address(ScAddress::Contract(XdrContractId(Hash(id.0))))
}

/// `ScVal::Address` for an account's `G…` address.
pub fn admin_scval(account: &Account) -> ScVal {
    ScVal::Address(ScAddress::Account(account.account_id()))
}

/// security: `[admin, numerator, denominator]`
pub fn security_ctor_args(admin: ScVal, numerator: u64, denominator: u64) -> Vec<ScVal> {
    vec![admin, ScVal::U64(numerator), ScVal::U64(denominator)]
}

/// verification: `[admin, security_addr]`
pub fn verification_ctor_args(admin: ScVal, security: ContractId) -> Vec<ScVal> {
    vec![admin, contract_scval(security)]
}

/// project_root: `[admin, security_addr, verification_addr, repo, vtype]`
pub fn project_root_ctor_args(
    admin: ScVal,
    security: ContractId,
    verification: ContractId,
    repo: String,
    verification_type: VerificationType,
) -> Vec<ScVal> {
    vec![
        admin,
        contract_scval(security),
        contract_scval(verification),
        repo.into_val(),
        ScVal::U32(verification_type as u32),
    ]
}

// ── Pipeline ─────────────────────────────────────────────────────────────────

fn security_wasm(variant: Variant) -> &'static str {
    match variant {
        Variant::Ethereum => SECP_SECURITY_WASM,
        Variant::Stellar => ED_SECURITY_WASM,
    }
}

fn verification_wasm(variant: Variant) -> &'static str {
    match variant {
        Variant::Ethereum => SECP_VERIFICATION_WASM,
        Variant::Stellar => ED_VERIFICATION_WASM,
    }
}

fn set_security(m: &mut StellarDeployManifest, id: ContractId) {
    match m.variant {
        Variant::Ethereum => m.contracts.secp256k1_security = Some(id),
        Variant::Stellar => m.contracts.ed25519_security = Some(id),
    }
}

fn set_verification(m: &mut StellarDeployManifest, id: ContractId) {
    match m.variant {
        Variant::Ethereum => m.contracts.secp256k1_verification = Some(id),
        Variant::Stellar => m.contracts.ed25519_verification = Some(id),
    }
}

/// Deploy one contract, retrying the whole upload+create on transient failure.
async fn deploy_one(
    env: &Env,
    account: &Account,
    wasm_dir: &Path,
    wasm_file: &str,
    ctor_args: Vec<ScVal>,
    retry_cfg: RetryConfig,
    label: &str,
) -> Result<ContractId> {
    let path = wasm_dir.join(wasm_file);
    let path = path
        .to_str()
        .ok_or_else(|| DeployerError::Config(format!("wasm path is not valid UTF-8: {wasm_file}")))?
        .to_string();

    eprintln!("=== deploying {label} ===");
    let deployed = retry(retry_cfg, || {
        let env = env.clone();
        let mut acct = account.clone();
        let wasm = path.clone();
        let ctor = ctor_args.clone();
        async move {
            let contract = Contract::new(&wasm, None)?;
            contract.deploy(&env, &mut acct, Some(ctor)).await
        }
    })
    .await?;

    let id = deployed.contract_id().ok_or(DeployerError::Soroban(
        wasi_soroban_rs::SorobanHelperError::ContractDeployedConfigsNotSet,
    ))?;
    eprintln!("{label}: {id}");
    Ok(id)
}

/// Run the idempotent deploy. Loads `manifest_path` if present (resume), skips
/// already-deployed slots, and persists after each successful deploy.
pub async fn deploy_pipeline(
    net: &NetworkConfig,
    account: &Account,
    params: &DeployParams,
    manifest_path: &Path,
    retry_cfg: RetryConfig,
) -> Result<StellarDeployManifest> {
    let env = net.env()?;
    let admin = account.account_id().to_string();
    let admin_addr = admin_scval(account);

    let mut manifest = match StellarDeployManifest::load_if_exists(manifest_path)? {
        Some(existing) => {
            if existing.variant != params.variant {
                return Err(DeployerError::Manifest(format!(
                    "existing manifest at {} is a `{}` deploy; refusing to deploy `{}` into it",
                    manifest_path.display(),
                    existing.variant,
                    params.variant
                )));
            }
            existing
        }
        None => StellarDeployManifest::new(admin.clone(), params.variant),
    };
    manifest.admin = admin;
    manifest.rpc_url = Some(net.rpc_url.clone());
    manifest.network_passphrase = Some(net.network_passphrase.clone());

    eprintln!("deploying as admin: {}", manifest.admin);

    // Step 1: security.
    let security_id = match manifest.security() {
        Some(id) => {
            eprintln!("=== reusing security ({id}) ===");
            id
        }
        None => {
            let (num, den) = params.threshold;
            let id = deploy_one(
                &env,
                account,
                &params.wasm_dir,
                security_wasm(params.variant),
                security_ctor_args(admin_addr.clone(), num, den),
                retry_cfg,
                "security",
            )
            .await?;
            set_security(&mut manifest, id);
            manifest.persist(manifest_path)?;
            id
        }
    };

    // Step 2: verification.
    let verification_id = match manifest.verification() {
        Some(id) => {
            eprintln!("=== reusing verification ({id}) ===");
            id
        }
        None => {
            let id = deploy_one(
                &env,
                account,
                &params.wasm_dir,
                verification_wasm(params.variant),
                verification_ctor_args(admin_addr.clone(), security_id),
                retry_cfg,
                "verification",
            )
            .await?;
            set_verification(&mut manifest, id);
            manifest.persist(manifest_path)?;
            id
        }
    };

    // Step 3: project_root (pinned to this variant's pipeline).
    match manifest.project_root() {
        Some(id) => eprintln!("=== reusing project-root ({id}) ==="),
        None => {
            let id = deploy_one(
                &env,
                account,
                &params.wasm_dir,
                PROJECT_ROOT_WASM,
                project_root_ctor_args(
                    admin_addr,
                    security_id,
                    verification_id,
                    params.project_spec_repo.clone(),
                    params.verification_type,
                ),
                retry_cfg,
                "project-root",
            )
            .await?;
            manifest.contracts.project_root = Some(id);
            manifest.persist(manifest_path)?;
        }
    }

    eprintln!("wrote deployment manifest to {}", manifest_path.display());
    Ok(manifest)
}
