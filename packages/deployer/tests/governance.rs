//! End-to-end governance-handover test against a live local Stellar Quickstart.
//!
//! Opt-in (`#[ignore]`). Ports `contracts/project-root/src/tests/governance.rs::
//! run_deployment_script` onto the deployer's typed fns (handlers omitted, per
//! PLAN.md §2/§5): deploy → add-signer (direct) → handover → owner accept-admin,
//! then assert the deployer has lost its privileges and the owner governs via
//! project_root.
//!
//! ```bash
//! RPC_URL=http://localhost:8000/rpc \
//! NETWORK_PASSPHRASE="Standalone Network ; February 2017" \
//! FRIENDBOT_URL=http://localhost:8000/friendbot \
//!   cargo test --test governance -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use warpdrive_client::project_root::VerificationType;
use warpdrive_deployer::config::NetworkConfig;
use warpdrive_deployer::deploy::{DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_pipeline};
use warpdrive_deployer::governance::{Target, accept_admin, handover};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer};

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the governance test"))
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}

#[tokio::test]
#[ignore = "requires a local Stellar Quickstart; run with --ignored"]
async fn handover_strips_deployer_privileges() {
    let net = NetworkConfig::new(env_or_skip("RPC_URL"), env_or_skip("NETWORK_PASSPHRASE"));
    let friendbot = std::env::var("FRIENDBOT_URL").ok();
    let retry_cfg = RetryConfig::default();

    let dir = tempfile::tempdir().unwrap();

    // Two funded identities: the deployer and the future owner (multisig stand-in).
    let deployer_key = dir.path().join("deployer.secret");
    keygen_and_fund(&net, friendbot.as_deref(), &deployer_key, retry_cfg)
        .await
        .expect("keygen deployer");
    let deployer = account_from_secret(&read_key_file(&deployer_key).unwrap()).unwrap();

    let owner_key = dir.path().join("owner.secret");
    let owner_address = keygen_and_fund(&net, friendbot.as_deref(), &owner_key, retry_cfg)
        .await
        .expect("keygen owner");
    let owner = account_from_secret(&read_key_file(&owner_key).unwrap()).unwrap();

    // Step 1-3: deploy + configure (add a signer directly, pre-handover).
    let manifest_path = dir.path().join("deploy.json");
    let manifest = deploy_pipeline(
        &net,
        &deployer,
        &DeployParams {
            variant: Variant::Ethereum,
            wasm_dir: wasm_dir(),
            project_spec_repo: DEFAULT_PROJECT_SPEC_REPO.to_string(),
            threshold: (2, 3),
            verification_type: VerificationType::Ethereum,
        },
        &manifest_path,
        retry_cfg,
    )
    .await
    .expect("deploy");

    let signer = format!("02{}", "11".repeat(32));
    add_signer(
        &net,
        &deployer,
        &manifest,
        Scheme::Secp256k1,
        &signer,
        100,
        false,
        retry_cfg,
    )
    .await
    .expect("direct add-signer pre-handover");

    // Step 4-5: handover (deployer proposes, project_root accepts downstream;
    // then project_root admin proposed to owner).
    handover(&net, &deployer, &manifest, &owner_address, retry_cfg)
        .await
        .expect("handover");

    // Owner finishes the project_root handover with their own key.
    accept_admin(&net, &owner, &manifest, Target::ProjectRoot, retry_cfg)
        .await
        .expect("owner accept-admin");

    // Step 6: the deployer can no longer touch security directly.
    let direct = add_signer(
        &net,
        &deployer,
        &manifest,
        Scheme::Secp256k1,
        &format!("02{}", "22".repeat(32)),
        50,
        false,
        retry_cfg,
    )
    .await;
    assert!(
        direct.is_err(),
        "deployer must not be able to add signers directly after handover"
    );

    // The owner governs through project_root's forwarder.
    add_signer(
        &net,
        &owner,
        &manifest,
        Scheme::Secp256k1,
        &format!("02{}", "33".repeat(32)),
        75,
        true,
        retry_cfg,
    )
    .await
    .expect("owner add-signer via project_root");
}
