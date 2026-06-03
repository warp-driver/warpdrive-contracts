//! Mocked deploy-pipeline tests. These drive the *real* `deploy_pipeline` /
//! `Contract::deploy` orchestration through `wasi_soroban_rs::mock_env`, so they
//! run hermetically in CI — no live network.
//!
//! Why mocked rather than a live local node: the contracts target protocol 26,
//! but Stellar Quickstart (incl. the compose-pinned image) currently maxes at
//! protocol 25 ("contract protocol number is newer than host"), so a local-node
//! deploy can't run yet. The live path is exercised against testnet (protocol
//! 26) by the `#[ignore]`d `tests/network.rs` / `tests/governance.rs`.
//!
//! The send response uses `TransactionMeta::V4`, so these also regression-guard
//! the protocol-23+ deploy bug (a V3-only parser returned no contract ID).

use std::fs;

use wasi_soroban_rs::wasi_stellar_rpc_client::{
    SimulateHostFunctionResultRaw, SimulateTransactionResponse,
};
use wasi_soroban_rs::xdr::{
    LedgerFootprint, Limits, SorobanResources, SorobanTransactionData, SorobanTransactionDataExt,
    VecM, WriteXdr,
};
use wasi_soroban_rs::{
    Account, ContractId, Env, mock_account_entry, mock_env, mock_signer1,
    mock_transaction_response_v4_with_return_value,
};

use warpdrive_client::project_root::VerificationType;
use warpdrive_deployer::config::NetworkConfig;
use warpdrive_deployer::deploy::{
    DeployParams, admin_scval, contract_scval, deploy_handler, deploy_pipeline,
};
use warpdrive_deployer::error::DeployerError;
use warpdrive_deployer::manifest::{StellarDeployManifest, Variant};
use warpdrive_deployer::retry::RetryConfig;

const WASM_NAMES: [&str; 7] = [
    "warpdrive_secp256k1_security.wasm",
    "warpdrive_secp256k1_verification.wasm",
    "warpdrive_ed25519_security.wasm",
    "warpdrive_ed25519_verification.wasm",
    "warpdrive_project_root.wasm",
    "warpdrive_ethereum_handler.wasm",
    "warpdrive_stellar_handler.wasm",
];

fn cid(n: u8) -> ContractId {
    ContractId([n; 32])
}

fn account() -> Account {
    Account::single(mock_signer1())
}

fn net() -> NetworkConfig {
    NetworkConfig::new("http://mock".to_string(), "Mock Net".to_string())
}

fn no_retry() -> RetryConfig {
    RetryConfig {
        max_retries: 1,
        sleep: std::time::Duration::from_millis(0),
    }
}

/// A temp dir holding dummy wasm for every contract. The bytes only need to
/// contain `__constructor` (so the constructor path runs) — the mock never
/// executes them.
fn wasm_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    for name in WASM_NAMES {
        fs::write(dir.path().join(name), b"\0asm __constructor dummy").unwrap();
    }
    dir
}

fn sim_response() -> SimulateTransactionResponse {
    let tx_data = SorobanTransactionData {
        ext: SorobanTransactionDataExt::V0,
        resources: SorobanResources {
            footprint: LedgerFootprint {
                read_only: VecM::default(),
                read_write: VecM::default(),
            },
            instructions: 0,
            disk_read_bytes: 0,
            write_bytes: 0,
        },
        resource_fee: 0,
    };
    SimulateTransactionResponse {
        min_resource_fee: 100,
        transaction_data: tx_data.to_xdr_base64(Limits::none()).unwrap(),
        // The result is the deployer's address: `deploy_pipeline`'s step-4
        // adoption and `deploy_handler`'s registration query `admin()` /
        // `pending_admin()`, which decode this. Deploys ignore the sim result
        // (the new contract id comes from the send meta), so this is harmless
        // for the create path.
        results: vec![SimulateHostFunctionResultRaw {
            auth: vec![],
            xdr: admin_scval(&account())
                .to_xdr_base64(Limits::none())
                .unwrap(),
        }],
        ..Default::default()
    }
}

/// A mock env where every `create_contract` resolves to `deployed_id` (via a
/// V4 transaction meta carrying the new contract address as the return value).
fn deploy_env(deployed_id: ContractId) -> Env {
    let entry = mock_account_entry(&account().account_id().to_string());
    let mut send = mock_transaction_response_v4_with_return_value(contract_scval(deployed_id));
    // Step-4 adoption / handler registration go through `tx_hash()`, which now
    // errors on a missing hash; the mock builder leaves it `None`, so set one.
    send.response.tx_hash = Some("mocktxhash".to_string());
    mock_env(Some(Ok(entry)), Some(Ok(sim_response())), Some(Ok(send)))
}

fn params(variant: Variant, wasm_dir: &tempfile::TempDir) -> DeployParams {
    DeployParams {
        variant,
        wasm_dir: wasm_dir.path().to_path_buf(),
        project_spec_repo: "ipfs://demo".to_string(),
        threshold: (2, 3),
        verification_type: match variant {
            Variant::Ethereum => VerificationType::Ethereum,
            Variant::Stellar => VerificationType::Stellar,
        },
    }
}

#[tokio::test]
async fn deploys_all_three_contracts_fresh() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");
    let env = deploy_env(cid(7));

    let m = deploy_pipeline(
        &env,
        &net(),
        &account(),
        &params(Variant::Ethereum, &wasm),
        &path,
        no_retry(),
    )
    .await
    .unwrap();

    // Every create resolves to the mocked id; the point is all three slots are
    // populated and the manifest is persisted.
    assert_eq!(m.contracts.secp256k1_security, Some(cid(7)));
    assert_eq!(m.contracts.secp256k1_verification, Some(cid(7)));
    assert_eq!(m.contracts.project_root, Some(cid(7)));
    assert_eq!(m.variant, Variant::Ethereum);
    assert_eq!(m.rpc_url.as_deref(), Some("http://mock"));

    // Checkpointed to disk.
    let reloaded = StellarDeployManifest::load(&path).unwrap();
    assert_eq!(reloaded, m);
}

#[tokio::test]
async fn reuses_contracts_when_manifest_complete() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    // Same admin as the deploying account, so the resume admin-guard passes.
    let mut pre = StellarDeployManifest::new(account().account_id().to_string(), Variant::Ethereum);
    pre.contracts.project_root = Some(cid(1));
    pre.contracts.secp256k1_security = Some(cid(2));
    pre.contracts.secp256k1_verification = Some(cid(3));
    pre.persist(&path).unwrap();

    // All pipeline contracts are present, so the three deploy steps are skipped.
    // Step-4 adoption still runs its admin queries + rotations against the mock;
    // the deployed_id (cid 99) must NOT appear, proving nothing was re-deployed.
    let env = deploy_env(cid(99));
    let m = deploy_pipeline(
        &env,
        &net(),
        &account(),
        &params(Variant::Ethereum, &wasm),
        &path,
        no_retry(),
    )
    .await
    .unwrap();

    assert_eq!(m.contracts.secp256k1_security, Some(cid(2)));
    assert_eq!(m.contracts.secp256k1_verification, Some(cid(3)));
    assert_eq!(m.contracts.project_root, Some(cid(1)));
}

#[tokio::test]
async fn resumes_partial_deploying_only_missing() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    // Only security already deployed.
    // Same admin as the deploying account, so the resume admin-guard passes.
    let mut pre = StellarDeployManifest::new(account().account_id().to_string(), Variant::Ethereum);
    pre.contracts.secp256k1_security = Some(cid(2));
    pre.persist(&path).unwrap();

    let env = deploy_env(cid(9));
    let m = deploy_pipeline(
        &env,
        &net(),
        &account(),
        &params(Variant::Ethereum, &wasm),
        &path,
        no_retry(),
    )
    .await
    .unwrap();

    // Existing security reused; verification + project_root freshly deployed.
    assert_eq!(m.contracts.secp256k1_security, Some(cid(2)));
    assert_eq!(m.contracts.secp256k1_verification, Some(cid(9)));
    assert_eq!(m.contracts.project_root, Some(cid(9)));
}

#[tokio::test]
async fn deploy_handler_records_handler_in_manifest() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    // Pipeline first so the manifest has a verification contract to point at.
    deploy_pipeline(
        &deploy_env(cid(7)),
        &net(),
        &account(),
        &params(Variant::Ethereum, &wasm),
        &path,
        no_retry(),
    )
    .await
    .unwrap();

    // Then the handler — a fourth create resolving to cid(8).
    let m = deploy_handler(
        &deploy_env(cid(8)),
        &account(),
        wasm.path(),
        &path,
        no_retry(),
    )
    .await
    .unwrap();
    assert_eq!(m.contracts.ethereum_handler, Some(cid(8)));
    assert_eq!(m.handler(), Some(cid(8)));
    // The stellar handler slot stays empty for an ethereum manifest.
    assert_eq!(m.contracts.stellar_handler, None);

    // Checkpointed to disk.
    let reloaded = StellarDeployManifest::load(&path).unwrap();
    assert_eq!(reloaded.contracts.ethereum_handler, Some(cid(8)));

    // Idempotent: a re-run reuses the existing handler (no new deploy) and
    // re-attempts the idempotent registration.
    let m2 = deploy_handler(
        &deploy_env(cid(8)),
        &account(),
        wasm.path(),
        &path,
        no_retry(),
    )
    .await
    .unwrap();
    assert_eq!(m2.contracts.ethereum_handler, Some(cid(8)));
}

#[tokio::test]
async fn deploy_handler_errors_when_pipeline_incomplete() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    // Manifest with only the security contract — no project_root (the handler's
    // admin) and no verification (its target) to bind to.
    // Same admin as the deploying account, so the resume admin-guard passes.
    let mut pre = StellarDeployManifest::new(account().account_id().to_string(), Variant::Ethereum);
    pre.contracts.secp256k1_security = Some(cid(2));
    pre.persist(&path).unwrap();

    let res = deploy_handler(
        &mock_env(None, None, None),
        &account(),
        wasm.path(),
        &path,
        no_retry(),
    )
    .await;
    assert!(matches!(res, Err(DeployerError::Manifest(_))), "{res:?}");
}

#[tokio::test]
async fn rejects_variant_mismatch_on_resume() {
    let wasm = wasm_dir();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    // Existing manifest is a stellar deploy …
    StellarDeployManifest::new("GOLD".to_string(), Variant::Stellar)
        .persist(&path)
        .unwrap();

    // … but we ask to deploy ethereum into it.
    let env = mock_env(None, None, None);
    let res = deploy_pipeline(
        &env,
        &net(),
        &account(),
        &params(Variant::Ethereum, &wasm),
        &path,
        no_retry(),
    )
    .await;

    assert!(matches!(res, Err(DeployerError::Manifest(_))), "{res:?}");
}
