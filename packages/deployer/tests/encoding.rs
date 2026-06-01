//! Pure (no-network) tests for constructor-arg encoding and key validation.

use warpdrive_client::project_root::VerificationType;
use wasi_soroban_rs::ContractId;
use wasi_soroban_rs::xdr::{ScAddress, ScVal};

use warpdrive_deployer::deploy::{
    contract_scval, project_root_ctor_args, security_ctor_args, verification_ctor_args,
};
use warpdrive_deployer::signers::{Scheme, parse_key};

fn cid(n: u8) -> ContractId {
    ContractId([n; 32])
}

#[test]
fn security_args_are_admin_then_two_u64() {
    let args = security_ctor_args(ScVal::Void, 2, 3);
    assert_eq!(args.len(), 3);
    assert!(matches!(args[0], ScVal::Void)); // admin passed through untouched
    assert_eq!(args[1], ScVal::U64(2));
    assert_eq!(args[2], ScVal::U64(3));
}

#[test]
fn verification_args_are_admin_then_contract_address() {
    let args = verification_ctor_args(ScVal::Void, cid(7));
    assert_eq!(args.len(), 2);
    assert!(matches!(args[0], ScVal::Void));
    assert!(matches!(args[1], ScVal::Address(ScAddress::Contract(_))));
    assert_eq!(args[1], contract_scval(cid(7)));
}

#[test]
fn project_root_args_have_correct_shape_and_order() {
    let args = project_root_ctor_args(
        ScVal::Void,
        cid(1),
        cid(2),
        "ipfs://repo".to_string(),
        VerificationType::Stellar,
    );
    assert_eq!(args.len(), 5);
    assert!(matches!(args[0], ScVal::Void)); // admin
    assert_eq!(args[1], contract_scval(cid(1))); // security
    assert_eq!(args[2], contract_scval(cid(2))); // verification
    assert!(matches!(args[3], ScVal::String(_))); // repo
    assert_eq!(args[4], ScVal::U32(2)); // Stellar => 2
}

#[test]
fn verification_type_encodes_as_u32() {
    let eth = project_root_ctor_args(
        ScVal::Void,
        cid(1),
        cid(2),
        "x".into(),
        VerificationType::Ethereum,
    );
    assert_eq!(eth[4], ScVal::U32(1));
}

#[test]
fn secp_accepts_33_bytes_rejects_32() {
    let good = "0x".to_string() + &"ab".repeat(33);
    assert_eq!(parse_key(Scheme::Secp256k1, &good).unwrap().len(), 33);

    let bad = "ab".repeat(32);
    assert!(parse_key(Scheme::Secp256k1, &bad).is_err());
}

#[test]
fn ed_accepts_32_bytes_rejects_33() {
    let good = "ab".repeat(32);
    assert_eq!(parse_key(Scheme::Ed25519, &good).unwrap().len(), 32);

    let bad = "ab".repeat(33);
    assert!(parse_key(Scheme::Ed25519, &bad).is_err());
}

#[test]
fn key_hex_prefix_is_optional() {
    let with = "0x".to_string() + &"cd".repeat(33);
    let without = "cd".repeat(33);
    assert_eq!(
        parse_key(Scheme::Secp256k1, &with).unwrap(),
        parse_key(Scheme::Secp256k1, &without).unwrap()
    );
}

#[test]
fn non_hex_key_is_rejected() {
    assert!(parse_key(Scheme::Secp256k1, "nothex").is_err());
}
