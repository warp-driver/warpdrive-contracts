//! Cross-version compatibility tests. Our client crate is built on
//! soroban-rs (stellar-xdr 23) while the contracts and warpdrive-shared are
//! built on soroban-sdk (stellar-xdr 25). Types can't be shared directly, so
//! each test serializes the client-side `ScVal` to XDR bytes and deserializes
//! it into the SDK-side `ScVal`, then uses `TryFromVal<Env, ScVal>` to reach
//! the typed soroban-sdk value. The reverse flow covers return values.
//!
//! If any of these tests break after a soroban-rs or contract-struct change,
//! the on-wire contract ABI is mismatched and the client will fail at
//! simulation time.

use soroban_rs::xdr::{Limits as ClientLimits, ScVal as ClientScVal, WriteXdr};
use soroban_sdk::xdr::{Limits as SdkLimits, ReadXdr, ScVal as SdkScVal};
use soroban_sdk::{Address, Bytes, BytesN, Env, String as SdkString, TryFromVal, TryIntoVal, Val};

use warpdrive_shared::interfaces::handler::{Ed25519SignatureData, SignatureData, XlmEnvelope};
use warpdrive_shared::interfaces::project_root::VerificationType as SharedVerificationType;

use crate::ethereum_handler::SignatureData as ClientSigData;
use crate::project_root::VerificationType as ClientVerificationType;
use crate::scval::IntoScValExt;
use crate::stellar_handler::Ed25519SignatureData as ClientEd25519SigData;

// ── XDR bridge ──────────────────────────────────────────────────────────

fn to_sdk(client: ClientScVal) -> SdkScVal {
    let bytes = client.to_xdr(ClientLimits::none()).expect("client xdr");
    SdkScVal::from_xdr(&bytes, SdkLimits::none()).expect("sdk parse")
}

fn from_sdk(sdk: &SdkScVal) -> ClientScVal {
    let bytes = soroban_sdk::xdr::WriteXdr::to_xdr(sdk, SdkLimits::none()).expect("sdk xdr");
    <ClientScVal as soroban_rs::xdr::ReadXdr>::from_xdr(&bytes, ClientLimits::none())
        .expect("client parse")
}

fn try_from_scval<T>(env: &Env, scval: &SdkScVal) -> T
where
    T: TryFromVal<Env, Val>,
    <T as TryFromVal<Env, Val>>::Error: std::fmt::Debug,
{
    let val: Val = Val::try_from_val(env, scval).expect("scval -> val");
    T::try_from_val(env, &val).expect("val -> typed")
}

fn to_sdk_scval<T>(env: &Env, v: T) -> SdkScVal
where
    T: TryIntoVal<Env, Val>,
    <T as TryIntoVal<Env, Val>>::Error: std::fmt::Debug,
{
    let val: Val = v.try_into_val(env).expect("typed -> val");
    SdkScVal::try_from_val(env, &val).expect("val -> scval")
}

/// Deterministic test account address. Uses a G-strkey derived from a fixed
/// 32-byte ed25519 public key.
fn account_address(env: &Env) -> Address {
    let pk = stellar_strkey::ed25519::PublicKey([7u8; 32]);
    Address::from_str(env, &pk.to_string())
}

// ── Args: client → contract ─────────────────────────────────────────────

#[test]
fn signature_data_decodes_into_contract_type() {
    let env = Env::default();
    let client = ClientSigData {
        signers: vec![[1u8; 33], [2u8; 33]],
        signatures: vec![[3u8; 65], [4u8; 65]],
        reference_block: 42,
    };
    let sdk_scval = to_sdk(client.into_scval().unwrap());

    let parsed: SignatureData = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.reference_block, 42);
    assert_eq!(parsed.signers.len(), 2);
    assert_eq!(parsed.signatures.len(), 2);
    assert_eq!(parsed.signers.get(0).unwrap().to_array(), [1u8; 33]);
    assert_eq!(parsed.signers.get(1).unwrap().to_array(), [2u8; 33]);
    assert_eq!(parsed.signatures.get(0).unwrap().to_array(), [3u8; 65]);
    assert_eq!(parsed.signatures.get(1).unwrap().to_array(), [4u8; 65]);
}

#[test]
fn ed25519_signature_data_decodes_into_contract_type() {
    let env = Env::default();
    let client = ClientEd25519SigData {
        signers: vec![[5u8; 32], [6u8; 32]],
        signatures: vec![[7u8; 64], [8u8; 64]],
        reference_block: 99,
    };
    let sdk_scval = to_sdk(client.into_scval().unwrap());

    let parsed: Ed25519SignatureData = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.reference_block, 99);
    assert_eq!(parsed.signers.get(0).unwrap().to_array(), [5u8; 32]);
    assert_eq!(parsed.signers.get(1).unwrap().to_array(), [6u8; 32]);
    assert_eq!(parsed.signatures.get(0).unwrap().to_array(), [7u8; 64]);
    assert_eq!(parsed.signatures.get(1).unwrap().to_array(), [8u8; 64]);
}

#[test]
fn envelope_bytes_decode_as_soroban_bytes() {
    let env = Env::default();
    let client: Vec<u8> = vec![0xde, 0xad, 0xbe, 0xef, 0x00, 0x01];
    let sdk_scval = to_sdk(client.clone().into_val_ext().unwrap());

    let parsed: Bytes = try_from_scval(&env, &sdk_scval);
    let round: std::vec::Vec<u8> = parsed.iter().collect();
    assert_eq!(round, client);
}

#[test]
fn event_id_decodes_as_bytesn_20() {
    let env = Env::default();
    let client: [u8; 20] = [9u8; 20];
    let sdk_scval = to_sdk(client.into_val_ext().unwrap());

    let parsed: BytesN<20> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn wasm_hash_decodes_as_bytesn_32() {
    use soroban_rs::IntoScVal;
    let env = Env::default();
    let client: [u8; 32] = [7u8; 32];
    let sdk_scval = to_sdk(client.into_val());

    let parsed: BytesN<32> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn version_string_decodes_as_soroban_string() {
    use soroban_rs::IntoScVal;
    let env = Env::default();
    let client = "v1.2.3".to_string();
    let sdk_scval = to_sdk(client.clone().into_val());

    let parsed: SdkString = try_from_scval(&env, &sdk_scval);
    let expected = SdkString::from_str(&env, &client);
    assert_eq!(parsed, expected);
}

#[test]
fn signer_pubkey_decodes_as_bytesn_33() {
    let env = Env::default();
    let client: [u8; 33] = [3u8; 33];
    let sdk_scval = to_sdk(client.into_val_ext().unwrap());

    let parsed: BytesN<33> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn signature_bytes_decode_as_bytesn_65() {
    let env = Env::default();
    let client: [u8; 65] = [2u8; 65];
    let sdk_scval = to_sdk(client.into_val_ext().unwrap());

    let parsed: BytesN<65> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn xlm_envelope_bytes_roundtrip_through_xdr() {
    // The handler contract parses the envelope via XDR — make sure a Vec<u8>
    // containing a valid XDR-serialized XlmEnvelope survives the round-trip
    // and decodes back to the same logical envelope on the contract side.
    let env = Env::default();
    let event_id: BytesN<20> = BytesN::from_array(&env, &[11u8; 20]);
    let ordering: BytesN<12> = BytesN::from_array(&env, &[12u8; 12]);
    let payload = Bytes::from_slice(&env, &[0xaa, 0xbb, 0xcc]);
    let envelope = XlmEnvelope {
        event_id: event_id.clone(),
        ordering: ordering.clone(),
        payload: payload.clone(),
    };

    use soroban_sdk::xdr::ToXdr;
    let envelope_bytes: std::vec::Vec<u8> = envelope.to_xdr(&env).iter().collect();

    let sdk_scval = to_sdk(envelope_bytes.into_val_ext().unwrap());
    let as_bytes: Bytes = try_from_scval(&env, &sdk_scval);

    use soroban_sdk::xdr::FromXdr;
    let parsed = XlmEnvelope::from_xdr(&env, &as_bytes).expect("parse envelope");
    assert_eq!(parsed.event_id, event_id);
    assert_eq!(parsed.ordering, ordering);
    assert_eq!(parsed.payload, payload);
}

// ── Return values: contract → client ────────────────────────────────────

#[test]
fn contract_address_decodes_to_client_contract_id() {
    use soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress};

    let env = Env::default();
    let contract_bytes = [0x42u8; 32];
    let strkey = stellar_strkey::Contract(contract_bytes).to_string();
    let sdk_addr = Address::from_str(&env, &strkey);
    let sdk_scval = to_sdk_scval(&env, sdk_addr);

    let client_scval = from_sdk(&sdk_scval);
    match client_scval {
        ClientScVal::Address(ScAddress::Contract(XdrContractId(Hash(b)))) => {
            assert_eq!(b, contract_bytes);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn account_address_decodes_to_client_account_id() {
    use soroban_rs::xdr::{AccountId, PublicKey, ScAddress, Uint256};

    let env = Env::default();
    let sdk_scval = to_sdk_scval(&env, account_address(&env));

    let client_scval = from_sdk(&sdk_scval);
    match client_scval {
        ClientScVal::Address(ScAddress::Account(AccountId(PublicKey::PublicKeyTypeEd25519(
            Uint256(bytes),
        )))) => {
            assert_eq!(bytes, [7u8; 32]);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn option_address_none_decodes_to_void() {
    let env = Env::default();
    let none: Option<Address> = None;
    let sdk_scval = to_sdk_scval(&env, none);

    assert!(matches!(from_sdk(&sdk_scval), ClientScVal::Void));
}

#[test]
fn option_address_some_decodes_to_address() {
    use soroban_rs::xdr::ScAddress;

    let env = Env::default();
    let some: Option<Address> = Some(account_address(&env));
    let sdk_scval = to_sdk_scval(&env, some);

    assert!(matches!(
        from_sdk(&sdk_scval),
        ClientScVal::Address(ScAddress::Account(_))
    ));
}

#[test]
fn contract_string_decodes_to_rust_string() {
    use soroban_rs::xdr::{ScString, ScVal};

    let env = Env::default();
    let expected = "soroban";
    let sdk_scval = to_sdk_scval(&env, SdkString::from_str(&env, expected));

    match from_sdk(&sdk_scval) {
        ScVal::String(ScString(s_m)) => {
            let bytes: std::vec::Vec<u8> = s_m.as_vec().clone().into();
            assert_eq!(String::from_utf8(bytes).unwrap(), expected);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn option_bytes_none_decodes_to_void() {
    let env = Env::default();
    let none: Option<Bytes> = None;
    let sdk_scval = to_sdk_scval(&env, none);

    assert!(matches!(from_sdk(&sdk_scval), ClientScVal::Void));
}

#[test]
fn option_bytes_some_decodes_to_bytes() {
    use soroban_rs::xdr::{ScBytes, ScVal};

    let env = Env::default();
    let some: Option<Bytes> = Some(Bytes::from_slice(&env, &[1, 2, 3, 4, 5]));
    let sdk_scval = to_sdk_scval(&env, some);

    match from_sdk(&sdk_scval) {
        ScVal::Bytes(ScBytes(b)) => {
            let v: std::vec::Vec<u8> = b.into();
            assert_eq!(v, vec![1u8, 2, 3, 4, 5]);
        }
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn u64_return_decodes_as_u64() {
    let env = Env::default();
    let sdk_scval = to_sdk_scval::<u64>(&env, 12345);

    match from_sdk(&sdk_scval) {
        ClientScVal::U64(w) => assert_eq!(w, 12345),
        other => panic!("unexpected: {:?}", other),
    }
}

fn client_verification_type(scval: ClientScVal) -> ClientVerificationType {
    match scval {
        ClientScVal::U32(1) => ClientVerificationType::Ethereum,
        ClientScVal::U32(2) => ClientVerificationType::Stellar,
        other => panic!("unexpected: {:?}", other),
    }
}

#[test]
fn verification_type_ethereum_decodes_to_client_enum() {
    let env = Env::default();
    let sdk_scval = to_sdk_scval(&env, SharedVerificationType::Ethereum);
    assert_eq!(
        client_verification_type(from_sdk(&sdk_scval)),
        ClientVerificationType::Ethereum,
    );
}

#[test]
fn verification_type_stellar_decodes_to_client_enum() {
    let env = Env::default();
    let sdk_scval = to_sdk_scval(&env, SharedVerificationType::Stellar);
    assert_eq!(
        client_verification_type(from_sdk(&sdk_scval)),
        ClientVerificationType::Stellar,
    );
}
