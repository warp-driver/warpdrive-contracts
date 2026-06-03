//! End-to-end governance lifecycle test against a live protocol-26 RPC (testnet).
//!
//! Opt-in (`#[ignore]`). Exercises the full deployer-driven lifecycle and the
//! ownership invariants it must uphold:
//!
//!   1. `deploy_pipeline` deploys security + verification + project_root AND
//!      adopts the downstreams (step 4) so project_root owns the whole pipeline.
//!   2. While the deployer is still project_root's admin, signer changes route
//!      *through* project_root.
//!   3. `handover` (step 5) rotates project_root's own admin to the owner; the
//!      owner accepts with their own key.
//!   4. After handover the deployer has no privileges, but the owner governs
//!      through project_root.
//!   5. Handler registration honours admin: a handler the deployer deploys
//!      *after* handover is NOT auto-registered (the deployer isn't
//!      project_root's admin) — the owner registers it explicitly.
//!
//! Transactions on testnet are slow, so state is asserted with cheap queries.
//!
//! ```bash
//! RPC_URL=https://soroban-testnet.stellar.org \
//! NETWORK_PASSPHRASE="Test SDF Network ; September 2015" \
//!   cargo test --test governance -- --ignored --nocapture
//! ```

use std::path::PathBuf;

use warpdrive_client::project_root::{ProjectRootClient, VerificationType};
use warpdrive_client::secp256k1_security::Secp256k1SecurityClient;
use warpdrive_client::warpdrive::WarpdriveClient;
use warpdrive_deployer::config::{NetworkConfig, client_configs};
use warpdrive_deployer::deploy::{
    DEFAULT_PROJECT_SPEC_REPO, DeployParams, deploy_handler, deploy_pipeline,
};
use warpdrive_deployer::governance::{Target, accept_admin, handover, register_handler};
use warpdrive_deployer::identity::{account_from_secret, keygen_and_fund, read_key_file};
use warpdrive_deployer::manifest::Variant;
use warpdrive_deployer::project_root::list_handlers;
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer};
use wasi_soroban_rs::ContractId;
use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress};

fn env_or_skip(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("set {name} to run the governance test"))
}

fn wasm_dir() -> PathBuf {
    std::env::var("WASM_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("../../target/wasm32v1-none/release"))
}

fn contract_scaddr(id: ContractId) -> ScAddress {
    ScAddress::Contract(XdrContractId(Hash(id.0)))
}

/// A secp256k1 pubkey of the form `02 11 11 …` (33 bytes) seeded by `tag`.
fn secp_key(tag: u8) -> [u8; 33] {
    let mut k = [tag; 33];
    k[0] = 0x02;
    k
}

#[tokio::test]
#[ignore = "requires a protocol-26 RPC (testnet); run with --ignored"]
async fn full_governance_lifecycle() {
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
    let deployer_addr = ScAddress::Account(deployer.account_id());

    let owner_key = dir.path().join("owner.secret");
    let owner_address = keygen_and_fund(&net, friendbot.as_deref(), &owner_key, retry_cfg)
        .await
        .expect("keygen owner");
    let owner = account_from_secret(&read_key_file(&owner_key).unwrap()).unwrap();
    let owner_addr = ScAddress::Account(owner.account_id());
    let env = net.env().unwrap();

    // ── 1. Deploy + adopt ───────────────────────────────────────────────────
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
    let verification = manifest.verification().unwrap();
    let pr = ProjectRootClient::new(client_configs(&env, &deployer, project_root));
    let sec = Secp256k1SecurityClient::new(client_configs(&env, &deployer, security));
    let ver_cfg = client_configs(&env, &deployer, verification);
    let ver = Secp256k1SecurityClient::new(ver_cfg); // only need its WarpdriveClient::admin

    // deploy_pipeline adopted the downstreams: both are owned by project_root,
    // and project_root is still owned by the deployer.
    assert_eq!(sec.admin().await.unwrap(), contract_scaddr(project_root));
    assert_eq!(ver.admin().await.unwrap(), contract_scaddr(project_root));
    assert_eq!(pr.admin().await.unwrap(), deployer_addr);

    // ── 2. Signer changes route through project_root (deployer is its admin) ─
    let signer = secp_key(0x11);
    add_signer(
        &env,
        &deployer,
        &manifest,
        Scheme::Secp256k1,
        &hex::encode(signer),
        100,
        true, // via project_root — the deployer is no longer security's admin
        retry_cfg,
    )
    .await
    .expect("add signer via project_root pre-handover");
    assert_eq!(sec.get_signer_weight(signer).await.unwrap(), 100);

    // ── 3. handover (step 5 only) + owner accepts ───────────────────────────
    handover(&env, &deployer, &manifest, &owner_address, retry_cfg)
        .await
        .expect("handover");
    accept_admin(&env, &owner, &manifest, Target::ProjectRoot, retry_cfg)
        .await
        .expect("owner accept-admin");
    assert_eq!(pr.admin().await.unwrap(), owner_addr);

    // ── 4. Post-handover privileges ─────────────────────────────────────────
    // The deployer can no longer govern security, even through project_root:
    // project_root.add_secp256k1_signer now requires the *owner's* auth, but the
    // deployer is the tx source, so simulation demands an Address credential the
    // SourceAccount-only client can't satisfy. It fails fast (this permanent
    // NotSupported error is non-retryable — see retry::Retryable, SOROBAN_RS.md).
    let deployer_attempt = add_signer(
        &env,
        &deployer,
        &manifest,
        Scheme::Secp256k1,
        &hex::encode(secp_key(0x22)),
        50,
        true,
        retry_cfg,
    )
    .await;
    assert!(
        deployer_attempt.is_err(),
        "deployer must not govern via project_root after handover"
    );

    // The owner can.
    let owner_signer = secp_key(0x33);
    add_signer(
        &env,
        &owner,
        &manifest,
        Scheme::Secp256k1,
        &hex::encode(owner_signer),
        75,
        true,
        retry_cfg,
    )
    .await
    .expect("owner add-signer via project_root");
    assert_eq!(sec.get_signer_weight(owner_signer).await.unwrap(), 75);

    // ── 5. Handler registration honours admin ───────────────────────────────
    // The deployer deploys a handler AFTER handover. It is born admin'd by
    // project_root, but the deployer is no longer project_root's admin, so
    // deploy_handler must NOT auto-register it.
    let manifest = deploy_handler(&env, &deployer, &wasm_dir(), &manifest_path, retry_cfg)
        .await
        .expect("deploy handler (post-handover, unregistered)");
    let handler = manifest.handler().expect("handler recorded in manifest");
    assert!(
        !list_handlers(&env, &deployer, &manifest)
            .await
            .unwrap()
            .contains(&handler),
        "a handler deployed by a non-admin deployer must not be auto-registered"
    );

    // The owner (project_root's admin) registers it explicitly.
    register_handler(&env, &owner, &manifest, retry_cfg)
        .await
        .expect("owner register-handler");
    assert!(
        list_handlers(&env, &owner, &manifest)
            .await
            .unwrap()
            .contains(&handler),
        "owner-registered handler must appear in list_handlers"
    );
}
