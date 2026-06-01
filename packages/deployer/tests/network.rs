//! End-to-end integration test against a live local Stellar Quickstart.
//!
//! Opt-in: `#[ignore]`d so it never runs in the default `cargo test`. Run with:
//!
//! ```bash
//! RPC_URL=http://localhost:8000/rpc \
//! NETWORK_PASSPHRASE="Standalone Network ; February 2017" \
//! FRIENDBOT_URL=http://localhost:8000/friendbot \
//!   cargo test --test network -- --ignored --nocapture
//! ```
//!
//! Walks the same flow the shell smoke test did: keygen → deploy (ethereum and
//! stellar, two files) → add-signer → set-threshold → project-spec-repo
//! get/set → get-ledger.

use std::path::PathBuf;

use warpdrive_client::project_root::VerificationType;
use warpdrive_deployer::config::NetworkConfig;
use warpdrive_deployer::deploy::{DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_pipeline};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::ledger::get_latest_ledger;
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::project_root::{get_project_spec_repo, set_project_spec_repo};
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer, set_threshold};

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the network test"))
}

#[tokio::test]
#[ignore = "requires a local Stellar Quickstart; run with --ignored"]
async fn full_pipeline() {
    let net = NetworkConfig::new(env_or_skip("RPC_URL"), env_or_skip("NETWORK_PASSPHRASE"));
    let friendbot = std::env::var("FRIENDBOT_URL").ok();
    let retry_cfg = RetryConfig::default();

    let dir = tempfile::tempdir().unwrap();
    let key_file = dir.path().join("deployer.secret");

    // keygen + fund.
    let address = keygen_and_fund(&net, friendbot.as_deref(), &key_file, retry_cfg)
        .await
        .expect("keygen");
    eprintln!("deployer address: {address}");
    let account = account_from_secret(&read_key_file(&key_file).unwrap()).unwrap();

    // deploy both pipelines into two files.
    let eth_path: PathBuf = dir.path().join("deploy-ethereum.json");
    let xlm_path: PathBuf = dir.path().join("deploy-stellar.json");

    let eth_manifest = deploy_pipeline(
        &net,
        &account,
        &DeployParams {
            variant: Variant::Ethereum,
            wasm_dir: wasm_dir(),
            project_spec_repo: DEFAULT_PROJECT_SPEC_REPO.to_string(),
            threshold: (2, 3),
            verification_type: VerificationType::Ethereum,
        },
        &eth_path,
        retry_cfg,
    )
    .await
    .expect("deploy ethereum");
    assert!(eth_manifest.project_root().is_some());

    let _xlm_manifest = deploy_pipeline(
        &net,
        &account,
        &DeployParams {
            variant: Variant::Stellar,
            wasm_dir: wasm_dir(),
            project_spec_repo: DEFAULT_PROJECT_SPEC_REPO.to_string(),
            threshold: (2, 3),
            verification_type: VerificationType::Stellar,
        },
        &xlm_path,
        retry_cfg,
    )
    .await
    .expect("deploy stellar");

    // add-signer (secp256k1, 33-byte key) against the ethereum manifest.
    let secp_key = format!("02{}", "11".repeat(32));
    let hash = add_signer(
        &net,
        &account,
        &eth_manifest,
        Scheme::Secp256k1,
        &secp_key,
        100,
        retry_cfg,
    )
    .await
    .expect("add signer");
    eprintln!("add_signer tx: {hash}");

    // set-threshold.
    set_threshold(
        &net,
        &account,
        &eth_manifest,
        Scheme::Secp256k1,
        1,
        2,
        retry_cfg,
    )
    .await
    .expect("set threshold");

    // project-spec-repo get/set.
    let repo_before = get_project_spec_repo(&net, &account, &eth_manifest)
        .await
        .expect("get repo");
    eprintln!("project_spec_repo: {repo_before}");
    set_project_spec_repo(&net, &account, &eth_manifest, "ipfs://updated", retry_cfg)
        .await
        .expect("set repo");
    let repo_after = get_project_spec_repo(&net, &account, &eth_manifest)
        .await
        .unwrap();
    assert_eq!(repo_after, "ipfs://updated");

    // get-ledger.
    let seq = get_latest_ledger(&net.rpc_url).await.expect("get ledger");
    assert!(seq > 0);
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}
