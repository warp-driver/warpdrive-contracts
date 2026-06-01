//! Thin CLI entrypoint: parse argv, build typed params, call a library
//! function, print its result. No business logic lives here.

use std::process::ExitCode;

use clap::Parser;

use warpdrive_deployer::cli::{Cli, Command};
use warpdrive_deployer::config::{NetworkConfig, resolve_wasm_dir};
use warpdrive_deployer::deploy::{
    DEFAULT_PROJECT_SPEC_REPO, DEFAULT_THRESHOLD, DeployParams, deploy_pipeline,
};
use warpdrive_deployer::error::Result;
use warpdrive_deployer::governance::{
    accept_admin, accept_contract_admin, handover, propose_admin,
};
use warpdrive_deployer::identity::{DEFAULT_KEY_FILE, keygen_and_fund, resolve_account};
use warpdrive_deployer::ledger::get_latest_ledger;
use warpdrive_deployer::manifest::{Variant, load as load_manifest};
use warpdrive_deployer::project_root::{get_project_spec_repo, set_project_spec_repo};
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{add_signer, remove_signer, set_threshold};

#[tokio::main]
async fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            ExitCode::FAILURE
        }
    }
}

async fn run(cli: Cli) -> Result<()> {
    let retry_cfg = RetryConfig::from_env();

    match cli.command {
        Command::Deploy(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let variant: Variant = args.variant.into();
            let params = DeployParams {
                variant,
                wasm_dir: resolve_wasm_dir(args.wasm_dir),
                project_spec_repo: args
                    .project_spec_repo
                    .unwrap_or_else(|| DEFAULT_PROJECT_SPEC_REPO.to_string()),
                threshold: resolve_threshold(
                    variant,
                    args.threshold_numerator,
                    args.threshold_denominator,
                ),
                verification_type: args
                    .verification_type
                    .map(Into::into)
                    .unwrap_or_else(|| variant.default_verification_type()),
            };
            deploy_pipeline(&net, &account, &params, &args.output_path, retry_cfg).await?;
            println!("{}", args.output_path.display());
        }

        Command::AddSigner(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let hash = add_signer(
                &net,
                &account,
                &manifest,
                args.scheme,
                &args.key,
                args.weight,
                args.via_project_root,
                retry_cfg,
            )
            .await?;
            println!("{hash}");
        }

        Command::RemoveSigner(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let hash = remove_signer(
                &net,
                &account,
                &manifest,
                args.scheme,
                &args.key,
                args.via_project_root,
                retry_cfg,
            )
            .await?;
            println!("{hash}");
        }

        Command::SetThreshold(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let hash = set_threshold(
                &net,
                &account,
                &manifest,
                args.scheme,
                args.numerator,
                args.denominator,
                args.via_project_root,
                retry_cfg,
            )
            .await?;
            println!("{hash}");
        }

        Command::GetProjectSpecRepo(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let repo = get_project_spec_repo(&net, &account, &manifest).await?;
            println!("{repo}");
        }

        Command::SetProjectSpecRepo(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let hash =
                set_project_spec_repo(&net, &account, &manifest, &args.repo, retry_cfg).await?;
            println!("{hash}");
        }

        Command::GetLedger(args) => {
            let seq = get_latest_ledger(&args.rpc_url).await?;
            println!("{seq}");
        }

        Command::Keygen(args) => {
            let net = NetworkConfig::new(args.rpc_url, String::new());
            let key_file = args
                .key_file
                .unwrap_or_else(|| std::path::PathBuf::from(DEFAULT_KEY_FILE));
            let address =
                keygen_and_fund(&net, args.friendbot_url.as_deref(), &key_file, retry_cfg).await?;
            println!("{address}");
        }

        Command::ProposeAdmin(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            propose_admin(
                &net,
                &account,
                &manifest,
                args.target,
                &args.new_admin,
                retry_cfg,
            )
            .await?;
            println!("proposed {} as admin of {}", args.new_admin, args.target);
        }

        Command::AcceptAdmin(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            accept_admin(&net, &account, &manifest, args.target, retry_cfg).await?;
            println!("accepted admin on {}", args.target);
        }

        Command::AcceptContractAdmin(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            let hash =
                accept_contract_admin(&net, &account, &manifest, args.target, retry_cfg).await?;
            println!("{hash}");
        }

        Command::Handover(args) => {
            let net = NetworkConfig::new(args.network.rpc_url, args.network.network_passphrase);
            let account = resolve_account(args.identity.secret, args.identity.secret_file)?;
            let manifest = load_manifest(&args.deploy_file.deploy_file)?;
            handover(&net, &account, &manifest, &args.owner, retry_cfg).await?;
        }
    }

    Ok(())
}

/// Resolve `(numerator, denominator)`: explicit flag → variant-specific env
/// (`SECP_*` / `ED_*`) → built-in default.
fn resolve_threshold(
    variant: Variant,
    numerator_flag: Option<u64>,
    denominator_flag: Option<u64>,
) -> (u64, u64) {
    let (num_env, den_env) = match variant {
        Variant::Ethereum => ("SECP_THRESHOLD_NUM", "SECP_THRESHOLD_DEN"),
        Variant::Stellar => ("ED_THRESHOLD_NUM", "ED_THRESHOLD_DEN"),
    };
    let numerator = numerator_flag
        .or_else(|| env_u64(num_env))
        .unwrap_or(DEFAULT_THRESHOLD.0);
    let denominator = denominator_flag
        .or_else(|| env_u64(den_env))
        .unwrap_or(DEFAULT_THRESHOLD.1);
    (numerator, denominator)
}

fn env_u64(name: &str) -> Option<u64> {
    std::env::var(name).ok().and_then(|v| v.parse().ok())
}
