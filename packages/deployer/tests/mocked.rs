//! Mocked-client tests (PLAN.md §9). These drive the network-touching typed
//! fns through `wasi_soroban_rs::mock_env` with canned RPC responses, so they
//! run in plain `cargo test` / CI — no live network. Enabled by R1 threading
//! `&Env` (rather than building it inside each fn).
//!
//! Two flavours:
//! - **guard paths** — argument/manifest validation that returns before any RPC
//!   call (mock provides no responses);
//! - **happy paths** — a read (`query`) and a write (`execute`) wired end-to-end
//!   through canned get_account / simulate / send responses.

use std::time::Duration;

use wasi_soroban_rs::wasi_stellar_rpc_client::{
    SimulateHostFunctionResultRaw, SimulateTransactionResponse,
};
use wasi_soroban_rs::xdr::{
    LedgerFootprint, Limits, ScVal, ScVec, SorobanResources, SorobanTransactionData,
    SorobanTransactionDataExt, VecM, WriteXdr,
};
use wasi_soroban_rs::{
    Account, ContractId, Env, IntoScVal, mock_account_entry, mock_env, mock_signer1,
    mock_transaction_response,
};

use warpdrive_deployer::deploy::contract_scval;
use warpdrive_deployer::error::DeployerError;
use warpdrive_deployer::governance::{Target, accept_contract_admin, propose_admin};
use warpdrive_deployer::manifest::{StellarDeployManifest, Variant};
use warpdrive_deployer::project_root::{get_project_spec_repo, list_handlers};
use warpdrive_deployer::retry::RetryConfig;
use warpdrive_deployer::signers::{Scheme, add_signer, set_threshold};

// ── Fixtures ─────────────────────────────────────────────────────────────────

fn account() -> Account {
    Account::single(mock_signer1())
}

fn cid(n: u8) -> ContractId {
    ContractId([n; 32])
}

/// A fully-populated single-variant manifest so the `require_*` guards pass.
fn manifest(variant: Variant) -> StellarDeployManifest {
    let mut m = StellarDeployManifest::new("GADMIN".to_string(), variant);
    match variant {
        Variant::Ethereum => {
            m.contracts.project_root = Some(cid(1));
            m.contracts.secp256k1_security = Some(cid(2));
            m.contracts.secp256k1_verification = Some(cid(3));
        }
        Variant::Stellar => {
            m.contracts.project_root = Some(cid(4));
            m.contracts.ed25519_security = Some(cid(5));
            m.contracts.ed25519_verification = Some(cid(6));
        }
    }
    m
}

/// One attempt, no sleep — guards never retry, happy paths succeed first try.
fn no_retry() -> RetryConfig {
    RetryConfig {
        max_retries: 1,
        sleep: Duration::from_millis(0),
    }
}

fn valid_secp_key() -> String {
    format!("02{}", "11".repeat(32)) // 33 bytes
}

/// A simulate response carrying one host-function result with `val` as the
/// return value (and no auth) — enough for the `query` read path.
fn sim_returning(val: ScVal) -> SimulateTransactionResponse {
    SimulateTransactionResponse {
        results: vec![SimulateHostFunctionResultRaw {
            auth: vec![],
            xdr: val.to_xdr_base64(Limits::none()).unwrap(),
        }],
        ..Default::default()
    }
}

/// A simulate response complete enough for the client's `execute` write path:
/// a result (empty auth), a non-zero fee, and valid Soroban transaction data.
fn sim_for_execute() -> SimulateTransactionResponse {
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
        results: vec![SimulateHostFunctionResultRaw {
            auth: vec![],
            xdr: ScVal::Void.to_xdr_base64(Limits::none()).unwrap(),
        }],
        ..Default::default()
    }
}

fn env_for_call(sim: SimulateTransactionResponse, with_send: bool) -> (Env, Account) {
    let account = account();
    let entry = mock_account_entry(&account.account_id().to_string());
    let send = with_send.then(|| Ok(mock_transaction_response()));
    let env = mock_env(Some(Ok(entry)), Some(Ok(sim)), send);
    (env, account)
}

// ── Guard paths (no RPC) ─────────────────────────────────────────────────────

#[tokio::test]
async fn add_signer_rejects_scheme_variant_mismatch() {
    let env = mock_env(None, None, None);
    let m = manifest(Variant::Stellar); // stellar manifest …
    let res = add_signer(
        &env,
        &account(),
        &m,
        Scheme::Secp256k1, // … but secp256k1 scheme
        &valid_secp_key(),
        1,
        false,
        no_retry(),
    )
    .await;
    assert!(matches!(res, Err(DeployerError::Manifest(_))), "{res:?}");
}

#[tokio::test]
async fn add_signer_rejects_bad_key_length() {
    let env = mock_env(None, None, None);
    let m = manifest(Variant::Ethereum);
    let res = add_signer(
        &env,
        &account(),
        &m,
        Scheme::Secp256k1,
        "abcd", // 2 bytes
        1,
        false,
        no_retry(),
    )
    .await;
    assert!(
        matches!(res, Err(DeployerError::InvalidArgument(_))),
        "{res:?}"
    );
}

#[tokio::test]
async fn add_signer_errors_when_security_absent() {
    let env = mock_env(None, None, None);
    let m = StellarDeployManifest::new("GADMIN".to_string(), Variant::Ethereum); // no contracts
    let res = add_signer(
        &env,
        &account(),
        &m,
        Scheme::Secp256k1,
        &valid_secp_key(),
        1,
        false,
        no_retry(),
    )
    .await;
    assert!(matches!(res, Err(DeployerError::Manifest(_))), "{res:?}");
}

#[tokio::test]
async fn accept_contract_admin_rejects_project_root_target() {
    let env = mock_env(None, None, None);
    let m = manifest(Variant::Ethereum);
    let res = accept_contract_admin(&env, &account(), &m, Target::ProjectRoot, no_retry()).await;
    assert!(
        matches!(res, Err(DeployerError::InvalidArgument(_))),
        "{res:?}"
    );
}

#[tokio::test]
async fn propose_admin_rejects_malformed_address() {
    let env = mock_env(None, None, None);
    let m = manifest(Variant::Ethereum);
    let res = propose_admin(
        &env,
        &account(),
        &m,
        Target::Security,
        "not-an-address",
        no_retry(),
    )
    .await;
    assert!(
        matches!(res, Err(DeployerError::InvalidArgument(_))),
        "{res:?}"
    );
}

// ── Happy paths (canned RPC) ─────────────────────────────────────────────────

#[tokio::test]
async fn get_project_spec_repo_decodes_simulation_result() {
    let (env, account) = env_for_call(sim_returning("ipfs://demo".to_string().into_val()), false);
    let m = manifest(Variant::Ethereum);
    let repo = get_project_spec_repo(&env, &account, &m).await.unwrap();
    assert_eq!(repo, "ipfs://demo");
}

#[tokio::test]
async fn list_handlers_decodes_simulation_result() {
    // Two contract addresses returned as a Vec, decoded to ContractIds.
    let handlers = ScVal::Vec(Some(ScVec(
        VecM::try_from(vec![
            contract_scval(ContractId([0xAB; 32])),
            contract_scval(ContractId([0xCD; 32])),
        ])
        .unwrap(),
    )));
    let (env, account) = env_for_call(sim_returning(handlers), false);
    let m = manifest(Variant::Ethereum);

    let result = list_handlers(&env, &account, &m).await.unwrap();
    assert_eq!(result, vec![ContractId([0xAB; 32]), ContractId([0xCD; 32])]);
}

#[tokio::test]
async fn list_handlers_decodes_empty_vec() {
    let (env, account) = env_for_call(
        sim_returning(ScVal::Vec(Some(ScVec(VecM::default())))),
        false,
    );
    let m = manifest(Variant::Ethereum);

    let result = list_handlers(&env, &account, &m).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn set_threshold_drives_execute_through_mock() {
    let (env, account) = env_for_call(sim_for_execute(), true);
    let m = manifest(Variant::Ethereum);
    let res = set_threshold(
        &env,
        &account,
        &m,
        Scheme::Secp256k1,
        1,
        2,
        false,
        no_retry(),
    )
    .await;
    assert!(res.is_ok(), "execute should succeed via the mock: {res:?}");
}

#[tokio::test]
async fn add_signer_via_project_root_drives_execute_through_mock() {
    // Proxy mode targets project_root but still runs the same execute path.
    let (env, account) = env_for_call(sim_for_execute(), true);
    let m = manifest(Variant::Ethereum);
    let res = add_signer(
        &env,
        &account,
        &m,
        Scheme::Secp256k1,
        &valid_secp_key(),
        50,
        true,
        no_retry(),
    )
    .await;
    assert!(
        res.is_ok(),
        "proxy execute should succeed via the mock: {res:?}"
    );
}
