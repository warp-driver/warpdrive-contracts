//! Golden-file round-trip tests for the shared deploy.json manifest, plus the
//! variant/scheme matching guard.

use warpdrive_deployer::manifest::{StellarDeployManifest, Variant};
use warpdrive_deployer::signers::Scheme;

const ETHEREUM_GOLDEN: &str = include_str!("golden/deploy-ethereum.json");
const STELLAR_GOLDEN: &str = include_str!("golden/deploy-stellar.json");

#[test]
fn ethereum_golden_round_trips() {
    let parsed: StellarDeployManifest = serde_json::from_str(ETHEREUM_GOLDEN).unwrap();
    assert_eq!(parsed.variant, Variant::Ethereum);
    assert!(parsed.contracts.project_root.is_some());
    assert!(parsed.contracts.secp256k1_security.is_some());
    assert!(parsed.contracts.secp256k1_verification.is_some());
    // No ed25519 slots in an ethereum file.
    assert!(parsed.contracts.ed25519_security.is_none());
    assert!(parsed.contracts.ed25519_verification.is_none());

    // Re-serialization is byte-identical to the golden file (guards field order
    // + the Option-skip behaviour).
    let reserialized = serde_json::to_string_pretty(&parsed).unwrap() + "\n";
    assert_eq!(reserialized, ETHEREUM_GOLDEN);
}

#[test]
fn stellar_golden_round_trips() {
    let parsed: StellarDeployManifest = serde_json::from_str(STELLAR_GOLDEN).unwrap();
    assert_eq!(parsed.variant, Variant::Stellar);
    assert!(parsed.contracts.ed25519_security.is_some());
    assert!(parsed.contracts.ed25519_verification.is_some());
    assert!(parsed.contracts.secp256k1_security.is_none());

    let reserialized = serde_json::to_string_pretty(&parsed).unwrap() + "\n";
    assert_eq!(reserialized, STELLAR_GOLDEN);
}

#[test]
fn accessors_resolve_per_variant() {
    let eth: StellarDeployManifest = serde_json::from_str(ETHEREUM_GOLDEN).unwrap();
    assert_eq!(eth.security(), eth.contracts.secp256k1_security);
    assert_eq!(eth.verification(), eth.contracts.secp256k1_verification);
    assert_eq!(eth.project_root(), eth.contracts.project_root);

    let xlm: StellarDeployManifest = serde_json::from_str(STELLAR_GOLDEN).unwrap();
    assert_eq!(xlm.security(), xlm.contracts.ed25519_security);
    assert_eq!(xlm.verification(), xlm.contracts.ed25519_verification);
}

#[test]
fn load_persist_round_trip_on_disk() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");

    let original: StellarDeployManifest = serde_json::from_str(ETHEREUM_GOLDEN).unwrap();
    original.persist(&path).unwrap();
    let reloaded = StellarDeployManifest::load(&path).unwrap();
    assert_eq!(original, reloaded);

    // load_if_exists is None for a missing file.
    let missing = dir.path().join("nope.json");
    assert!(
        StellarDeployManifest::load_if_exists(&missing)
            .unwrap()
            .is_none()
    );
}

#[test]
fn partial_manifest_round_trips() {
    // A mid-deploy checkpoint with only the security contract present must
    // serialize + parse cleanly (idempotency depends on it).
    let mut m = StellarDeployManifest::new("GABC".to_string(), Variant::Ethereum);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("deploy.json");
    m.contracts.secp256k1_security = Some(wasi_soroban_rs::ContractId([9; 32]));
    m.persist(&path).unwrap();

    let reloaded = StellarDeployManifest::load(&path).unwrap();
    assert_eq!(
        reloaded.contracts.secp256k1_security,
        m.contracts.secp256k1_security
    );
    assert!(reloaded.contracts.project_root.is_none());
    assert!(reloaded.contracts.secp256k1_verification.is_none());
}

#[test]
fn scheme_must_match_variant() {
    // A stellar manifest's variant doesn't match the secp256k1 scheme, so
    // `add-signer --scheme secp256k1` against it is a usage error.
    let xlm: StellarDeployManifest = serde_json::from_str(STELLAR_GOLDEN).unwrap();
    assert_ne!(Scheme::Secp256k1.variant(), xlm.variant);
    assert_eq!(Scheme::Ed25519.variant(), xlm.variant);
}
