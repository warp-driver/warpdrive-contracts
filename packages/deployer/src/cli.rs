//! clap definitions. Kept declarative; all behaviour lives in the typed
//! library functions dispatched from `main.rs`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};
use warpdrive_client::project_root::VerificationType;

use crate::manifest::Variant;

/// Native deployer for the WarpDrive Stellar contracts. Replaces the old shell
/// and stellar-cli middleware: every subcommand drives `warpdrive-client` and
/// `wasi-soroban-rs` directly.
#[derive(Debug, Parser)]
#[command(name = "warpdrive-deployer", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Deploy a contract pipeline (security + verification + project-root) and
    /// write a JSON manifest. One pipeline per file; run twice for both.
    Deploy(DeployArgs),
    /// Register or update a signer on the matching security contract.
    AddSigner(AddSignerArgs),
    /// Remove a signer from the matching security contract.
    RemoveSigner(RemoveSignerArgs),
    /// Update the threshold (numerator/denominator) on the security contract.
    SetThreshold(SetThresholdArgs),
    /// Read the project_spec_repo URL from the project-root contract.
    GetProjectSpecRepo(ProjectSpecRepoArgs),
    /// Update the project_spec_repo URL on the project-root contract (admin).
    SetProjectSpecRepo(SetProjectSpecRepoArgs),
    /// Print the current ledger sequence from the configured RPC.
    GetLedger(GetLedgerArgs),
    /// Generate (if needed) and friendbot-fund a deployer identity keyfile.
    Keygen(KeygenArgs),
}

/// RPC coordinates needed for signing transactions.
#[derive(Debug, Args)]
pub struct NetworkArgs {
    #[arg(long, env = "RPC_URL")]
    pub rpc_url: String,
    #[arg(long, env = "NETWORK_PASSPHRASE")]
    pub network_passphrase: String,
}

/// BYOK identity selection (precedence: --secret → --secret-file → env → default
/// keyfile; see identity::resolve_secret).
#[derive(Debug, Args)]
pub struct IdentityArgs {
    /// A funded Stellar secret (`S…`).
    #[arg(long)]
    pub secret: Option<String>,
    /// Path to a file holding a funded Stellar secret.
    #[arg(long)]
    pub secret_file: Option<PathBuf>,
}

/// Manifest file selecting the deployment to operate on.
#[derive(Debug, Args)]
pub struct DeployFileArg {
    #[arg(long)]
    pub deploy_file: PathBuf,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum VariantArg {
    Ethereum,
    Stellar,
}

impl From<VariantArg> for Variant {
    fn from(v: VariantArg) -> Self {
        match v {
            VariantArg::Ethereum => Variant::Ethereum,
            VariantArg::Stellar => Variant::Stellar,
        }
    }
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum VerificationTypeArg {
    Ethereum,
    Stellar,
}

impl From<VerificationTypeArg> for VerificationType {
    fn from(v: VerificationTypeArg) -> Self {
        match v {
            VerificationTypeArg::Ethereum => VerificationType::Ethereum,
            VerificationTypeArg::Stellar => VerificationType::Stellar,
        }
    }
}

#[derive(Debug, Args)]
pub struct DeployArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    /// Where to write (and resume) the deployment manifest.
    #[arg(long)]
    pub output_path: PathBuf,
    /// Which pipeline to deploy.
    #[arg(long, value_enum, default_value_t = VariantArg::Ethereum)]
    pub variant: VariantArg,
    /// Directory holding the contract wasm (default: WASM_DIR or /warpdrive/wasm).
    #[arg(long)]
    pub wasm_dir: Option<PathBuf>,
    /// project_spec_repo URI baked into project-root.
    #[arg(long, env = "PROJECT_SPEC_REPO")]
    pub project_spec_repo: Option<String>,
    /// Threshold numerator (default 2, or {SECP,ED}_THRESHOLD_NUM).
    #[arg(long)]
    pub threshold_numerator: Option<u64>,
    /// Threshold denominator (default 3, or {SECP,ED}_THRESHOLD_DEN).
    #[arg(long)]
    pub threshold_denominator: Option<u64>,
    /// Override project-root's verification_type (default: matches --variant).
    #[arg(long, value_enum)]
    pub verification_type: Option<VerificationTypeArg>,
}

#[derive(Debug, Args)]
pub struct AddSignerArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    #[command(flatten)]
    pub deploy_file: DeployFileArg,
    #[arg(long, value_enum)]
    pub scheme: crate::signers::Scheme,
    /// Hex public key (`0x`-prefix optional).
    #[arg(long)]
    pub key: String,
    #[arg(long)]
    pub weight: u64,
}

#[derive(Debug, Args)]
pub struct RemoveSignerArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    #[command(flatten)]
    pub deploy_file: DeployFileArg,
    #[arg(long, value_enum)]
    pub scheme: crate::signers::Scheme,
    #[arg(long)]
    pub key: String,
}

#[derive(Debug, Args)]
pub struct SetThresholdArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    #[command(flatten)]
    pub deploy_file: DeployFileArg,
    #[arg(long, value_enum)]
    pub scheme: crate::signers::Scheme,
    #[arg(long)]
    pub numerator: u64,
    #[arg(long)]
    pub denominator: u64,
}

#[derive(Debug, Args)]
pub struct ProjectSpecRepoArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    #[command(flatten)]
    pub deploy_file: DeployFileArg,
}

#[derive(Debug, Args)]
pub struct SetProjectSpecRepoArgs {
    #[command(flatten)]
    pub network: NetworkArgs,
    #[command(flatten)]
    pub identity: IdentityArgs,
    #[command(flatten)]
    pub deploy_file: DeployFileArg,
    #[arg(long)]
    pub repo: String,
}

#[derive(Debug, Args)]
pub struct GetLedgerArgs {
    #[arg(long, env = "RPC_URL")]
    pub rpc_url: String,
}

#[derive(Debug, Args)]
pub struct KeygenArgs {
    #[arg(long, env = "RPC_URL")]
    pub rpc_url: String,
    /// Where to read/write the secret (default /out/.keys/deployer.secret).
    #[arg(long, env = "KEY_FILE")]
    pub key_file: Option<PathBuf>,
    /// Explicit friendbot endpoint; else derived via getNetwork.
    #[arg(long, env = "FRIENDBOT_URL")]
    pub friendbot_url: Option<String>,
}
