//! End-to-end integration test against a live Stellar node.
//!
//! Opt-in: `#[ignore]`d so it never runs in the default `cargo test`. Prefer
//! `task test-deployer-it`. The contracts target **protocol 26**, so this needs
//! a protocol-26 RPC — testnet today. Local Stellar Quickstart (incl. the
//! compose-pinned image) currently maxes at protocol 25, so its `--local` node
//! rejects the wasm ("contract protocol number is newer than host"); the
//! hermetic deploy coverage lives in `tests/deploy.rs` (mock_env) instead.
//!
//! ```bash
//! RPC_URL=https://soroban-testnet.stellar.org \
//! NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
//!   cargo test --test network -- --ignored --nocapture
//! ```
//!
//! Covers the "deploy and operate while the deployer is still project_root's
//! admin" path (no handover — that's `tests/governance.rs`):
//! keygen → deploy ethereum + stellar pipelines (each adopts its downstreams to
//! project_root) → verify ownership by query → add-signer / set-threshold
//! *through project_root* → project-spec-repo get/set → deploy-handler
//! (auto-registers, since the deployer is project_root's admin) → many queries
//! → get-ledger. Transactions on testnet are slow, so we lean on cheap queries
//! to assert state.

use std::path::PathBuf;

use warpdrive_client::project_root::{ProjectRootClient, VerificationType};
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::warpdrive::WarpdriveClient;
use warpdrive_deployer::config::{NetworkConfig, client_configs};
use warpdrive_deployer::deploy::{
    DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_handler, deploy_pipeline,
};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::ledger::get_latest_ledger;
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::project_root::{
    get_project_spec_repo, list_handlers, set_project_spec_repo,
};
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer, set_threshold};
use wasi_soroban_rs::ContractId;
use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress};

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the network test"))
}

/// The `C…` contract id as an `ScAddress`, for comparing against `admin()`.
fn contract_scaddr(id: ContractId) -> ScAddress {
    ScAddress::Contract(XdrContractId(Hash(id.0)))
}

#[tokio::test]
#[ignore = "requires a protocol-26 RPC (testnet); run with --ignored"]
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
    let deployer_addr = ScAddress::Account(account.account_id());
    let env = net.env().unwrap();

    // deploy both pipelines into two files (each adopts its downstreams).
    let eth_path: PathBuf = dir.path().join("deploy-ethereum.json");
    let xlm_path: PathBuf = dir.path().join("deploy-stellar.json");

    let eth_manifest = deploy_pipeline(
        &env,
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
    let project_root = eth_manifest.project_root().expect("project_root");
    let security = eth_manifest.security().expect("security");
    let verification = eth_manifest.verification().expect("verification");

    let _xlm_manifest = deploy_pipeline(
        &env,
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

    // ── Ownership: deploy_pipeline adopted the downstreams (queries) ────────
    let pr = ProjectRootClient::new(client_configs(&env, &account, project_root));
    let sec = Secp256k1SecurityClient::new(client_configs(&env, &account, security));
    assert_eq!(
        WarpdriveClient::admin(&sec).await.expect("security admin"),
        contract_scaddr(project_root),
        "security must be owned by project_root after deploy"
    );
    assert_eq!(
        pr.admin().await.expect("project_root admin"),
        deployer_addr,
        "project_root is still owned by the deployer (no handover here)"
    );
    // Constructor wiring is queryable on project_root.
    assert_eq!(pr.security_contract().await.unwrap(), security);
    assert_eq!(pr.verification_contract().await.unwrap(), verification);
    assert_eq!(
        pr.verification_type().await.unwrap(),
        VerificationType::Ethereum
    );

    // ── add-signer THROUGH project_root (deployer is no longer security's
    //    admin, but is project_root's) ───────────────────────────────────────
    let mut signer = [0x11u8; 33];
    signer[0] = 0x02;
    let signer_hex = hex::encode(signer);
    add_signer(
        &env,
        &account,
        &eth_manifest,
        Scheme::Secp256k1,
        &signer_hex,
        100,
        true, // via project_root
        retry_cfg,
    )
    .await
    .expect("add signer via project_root");
    assert_eq!(
        sec.get_signer_weight(signer).await.unwrap(),
        100,
        "signer weight should be set"
    );
    assert_eq!(sec.get_total_weight().await.unwrap(), 100);
    assert_eq!(sec.list_signers().await.unwrap().len(), 1);

    // ── set-threshold through project_root ──────────────────────────────────
    set_threshold(
        &env,
        &account,
        &eth_manifest,
        Scheme::Secp256k1,
        1,
        2,
        true,
        retry_cfg,
    )
    .await
    .expect("set threshold via project_root");
    assert_eq!(sec.threshold_numerator().await.unwrap(), 1);
    assert_eq!(sec.threshold_denominator().await.unwrap(), 2);

    // ── project-spec-repo get/set (admin write on project_root) ─────────────
    let repo_before = get_project_spec_repo(&env, &account, &eth_manifest)
        .await
        .expect("get repo");
    eprintln!("project_spec_repo: {repo_before}");
    set_project_spec_repo(&env, &account, &eth_manifest, "ipfs://updated", retry_cfg)
        .await
        .expect("set repo");
    assert_eq!(
        get_project_spec_repo(&env, &account, &eth_manifest)
            .await
            .unwrap(),
        "ipfs://updated"
    );

    // ── deploy-handler: born admin'd by project_root, auto-registered because
    //    the deployer is still project_root's admin ──────────────────────────
    let eth_manifest = deploy_handler(&env, &account, &wasm_dir(), &eth_path, retry_cfg)
        .await
        .expect("deploy handler");
    let handler = eth_manifest
        .handler()
        .expect("handler recorded in manifest");
    eprintln!("handler: {handler}");
    assert!(
        list_handlers(&env, &account, &eth_manifest)
            .await
            .expect("list handlers")
            .contains(&handler),
        "auto-registered handler must appear in list_handlers"
    );

    // get-ledger (cheap query).
    let seq = get_latest_ledger(&net.rpc_url).await.expect("get ledger");
    assert!(seq > 0);
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}
