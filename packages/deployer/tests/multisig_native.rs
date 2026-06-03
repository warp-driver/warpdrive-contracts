//! Reproduction test: a **native (classic) multisig** `project_root` owner and
//! the `wasi-soroban-rs` Address-credential gap (see `SOROBAN_RS.md`).
//!
//! Opt-in (`#[ignore]`). Runs against a live protocol-26 RPC (testnet). Prefer
//! `task test-deployer-multisig`.
//!
//! ## What this proves
//!
//! A native multisig is a plain `G…` account that has been given extra signers
//! and raised thresholds via `SetOptions` (here: master weight 1 + one cosigner
//! weight 1, medium threshold 2 — a 2-of-2). Two facts matter:
//!
//!   * **A native multisig CAN govern when it is the transaction source.** When
//!     the multisig signs its own transaction, simulation reports its
//!     `require_auth` as a `SourceAccount` credential (signer == source), and
//!     the client simply attaches every key's signature. So the owner finishing
//!     the handover (`accept_admin`) works today — step 3 below is green.
//!
//! ```bash
//! RPC_URL=https://soroban-testnet.stellar.org \
//! NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
//!   cargo test --test multisig_native -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use warpdrive_client::project_root::{ProjectRootClient, VerificationType};
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::warpdrive::WarpdriveClient;
use warpdrive_deployer::config::{NetworkConfig, client_configs};
use warpdrive_deployer::deploy::{DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_pipeline};
use warpdrive_deployer::governance::{Target, accept_admin, handover};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer};
use wasi_soroban_rs::xdr::ScAddress;
use wasi_soroban_rs::{Account, AccountConfig, Signer};

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the multisig test"))
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}

/// A secp256k1 pubkey of the form `02 11 11 …` (33 bytes) seeded by `tag`.
fn secp_key(tag: u8) -> [u8; 33] {
    let mut k = [tag; 33];
    k[0] = 0x02;
    k
}

#[tokio::test]
#[ignore = "requires a protocol-26 RPC (testnet); run with --ignored"]
async fn native_multisig_owner_governance() {
    let net = NetworkConfig::new(env_or_skip("RPC_URL"), env_or_skip("NETWORK_PASSPHRASE"));
    let friendbot = std::env::var("FRIENDBOT_URL").ok();
    let retry_cfg = RetryConfig::default();

    let dir = tempfile::tempdir().unwrap();

    // The deployer (tx source for the deploy + the relayer in step 4).
    let deployer_key = dir.path().join("deployer.secret");
    keygen_and_fund(&net, friendbot.as_deref(), &deployer_key, retry_cfg)
        .await
        .expect("keygen deployer");
    let deployer = account_from_secret(&read_key_file(&deployer_key).unwrap()).unwrap();

    // The future owner's master key (a funded `G…` account).
    let owner_key = dir.path().join("owner.secret");
    let owner_address = keygen_and_fund(&net, friendbot.as_deref(), &owner_key, retry_cfg)
        .await
        .expect("keygen owner");
    let owner_master = account_from_secret(&read_key_file(&owner_key).unwrap()).unwrap();
    let owner_id = owner_master.account_id();
    let owner_addr = ScAddress::Account(owner_id.clone());
    let master_signer = owner_master.signers()[0].clone();

    let env = net.env().unwrap();

    // ── 0. Turn the owner account into a real native multisig ────────────────
    // master weight 1 + one cosigner weight 1, medium threshold 2 (a 2-of-2).
    // The initial SetOptions is a high-threshold op evaluated against the
    // *pre-change* thresholds (0), so the master key alone authorizes it.
    let cosigner = Signer::from(&[0xC0u8; 32]);
    let config = AccountConfig::new()
        .with_master_weight(1)
        .with_thresholds(1, 2, 2)
        .add_signer(cosigner.public_key(), 1);
    let configure_tx = owner_master
        .configure(&env, config)
        .await
        .expect("build owner set-options (native multisig)");
    env.send_transaction(&configure_tx)
        .await
        .expect("submit owner set-options (native multisig)");
    // The multisig account: signs every transaction with both keys (weight 2,
    // meeting the medium threshold required for contract invocations).
    let owner_multisig = Account::multisig(owner_id.clone(), vec![master_signer, cosigner]);

    // ── 1. Deploy + adopt (deployer is project_root's admin) ─────────────────
    let manifest_path = dir.path().join("deploy.json");
    let manifest = deploy_pipeline(
        &env,
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

    let project_root = manifest.project_root().unwrap();
    let security = manifest.security().unwrap();
    let pr = ProjectRootClient::new(client_configs(&env, &deployer, project_root));
    let sec = Secp256k1SecurityClient::new(client_configs(&env, &deployer, security));

    // ── 2. Hand project_root over to the native multisig owner ───────────────
    handover(&env, &deployer, &manifest, &owner_address, retry_cfg)
        .await
        .expect("propose native multisig owner as project_root admin");

    // ── 3. The owner finishes the handover *as its own source* (works today) ─
    // The multisig signs accept_admin with both keys; simulation sees signer ==
    // source and emits a SourceAccount credential, so the client is happy.
    accept_admin(
        &env,
        &owner_multisig,
        &manifest,
        Target::ProjectRoot,
        retry_cfg,
    )
    .await
    .expect("native multisig self-sourced accept-admin");
    assert_eq!(
        pr.admin().await.unwrap(),
        owner_addr,
        "native multisig owner should now be project_root's admin"
    );

    // ── 4. A relayer governs *on the multisig's behalf* (the blocked flow) ───
    // This is how a multisig is driven in practice: a coordinator submits the
    // transaction and the multisig authorizes via an Address-credential auth
    // entry. Here the deployer is the relayer (tx source) while project_root's
    // admin is the owner, so simulation demands Address(owner) — currently
    // rejected. Expected to fail until the upstream auth-entry signing lands.
    let relayer_signer = secp_key(0x55);
    add_signer(
        &env,
        &owner_multisig,
        &manifest,
        Scheme::Secp256k1,
        &hex::encode(relayer_signer),
        42,
        true, // via project_root, whose admin is the multisig owner
        retry_cfg,
    )
    .await
    .expect("owner multisig cannot call add-signer");

    // Post-fix assertions (only reached once the gap is closed).
    assert_eq!(
        sec.get_signer_weight(relayer_signer).await.unwrap(),
        42,
        "relayed signer change should be applied once Address auth is supported"
    );
}
