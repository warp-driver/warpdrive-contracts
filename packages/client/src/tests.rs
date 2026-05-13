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

use soroban_sdk::xdr::{Limits as SdkLimits, ReadXdr, ScVal as SdkScVal};
use soroban_sdk::{Address, Bytes, BytesN, Env, String as SdkString, TryFromVal, TryIntoVal, Val};
use wasi_soroban_rs::xdr::{Limits as ClientLimits, ScVal as ClientScVal, WriteXdr};

use warpdrive_shared::interfaces::handler::{
    Ed25519SignatureData, MessageWithId as SharedMessageWithId, SignatureData, XlmEnvelope,
};
use warpdrive_shared::interfaces::project_root::VerificationType as SharedVerificationType;
use warpdrive_shared::interfaces::security::{
    Ed25519SignerInfo as SharedEd25519SignerInfo, SignerInfo as SharedSignerInfo,
};

use crate::ed25519_security::{
    Ed25519SignerInfo as ClientEd25519SignerInfo, decode_signer_info as decode_ed25519_signer_info,
};
use crate::ethereum_handler::SignatureData as ClientSigData;
use crate::message_with_id::MessageWithId as ClientMessageWithId;
use crate::project_root::VerificationType as ClientVerificationType;
use crate::scval::IntoScValExt;
use crate::stellar_handler::Ed25519SignatureData as ClientEd25519SigData;
use crate::xlm_envelope::XlmEnvelope as ClientXlmEnvelope;

// ── XDR bridge ──────────────────────────────────────────────────────────

fn to_sdk(client: ClientScVal) -> SdkScVal {
    let bytes = client.to_xdr(ClientLimits::none()).expect("client xdr");
    SdkScVal::from_xdr(&bytes, SdkLimits::none()).expect("sdk parse")
}

fn from_sdk(sdk: &SdkScVal) -> ClientScVal {
    let bytes = soroban_sdk::xdr::WriteXdr::to_xdr(sdk, SdkLimits::none()).expect("sdk xdr");
    <ClientScVal as wasi_soroban_rs::xdr::ReadXdr>::from_xdr(&bytes, ClientLimits::none())
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
    use wasi_soroban_rs::IntoScVal;
    let env = Env::default();
    let client: [u8; 32] = [7u8; 32];
    let sdk_scval = to_sdk(client.into_val());

    let parsed: BytesN<32> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn version_string_decodes_as_soroban_string() {
    use wasi_soroban_rs::IntoScVal;
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

#[test]
fn xlm_envelope_new_encode_decode_roundtrip_no_env() {
    // Round-trip through the convenience methods with `env: None` — exercises
    // both the plain-bytes API and the implicit `Env::default()` fallback.
    let payload = std::vec![0xaa, 0xbb, 0xcc];
    let event_id = [11u8; 20];
    let ordering = [12u8; 12];

    let bytes = XlmEnvelope::new(None, payload.clone(), event_id, ordering).encode(None);
    let parsed = XlmEnvelope::decode(None, &bytes).expect("decode envelope");

    assert_eq!(parsed.event_id.to_array(), event_id);
    assert_eq!(parsed.ordering.to_array(), ordering);
    let payload_out: std::vec::Vec<u8> = parsed.payload.iter().collect();
    assert_eq!(payload_out, payload);
}

// ── Return values: contract → client ────────────────────────────────────

#[test]
fn contract_address_decodes_to_client_contract_id() {
    use wasi_soroban_rs::xdr::{ContractId as XdrContractId, Hash, ScAddress};

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
    use wasi_soroban_rs::xdr::{AccountId, PublicKey, ScAddress, Uint256};

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
    use wasi_soroban_rs::xdr::ScAddress;

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
    use wasi_soroban_rs::xdr::{ScString, ScVal};

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
    use wasi_soroban_rs::xdr::{ScBytes, ScVal};

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

// ── MessageWithId: shape parity between client and shared ───────────────

#[test]
fn message_with_id_client_to_shared_roundtrip() {
    // Client-encoded XDR bytes must decode into the shared `MessageWithId`
    // contracttype with the same field values.
    let env = Env::default();
    let client = ClientMessageWithId {
        trigger_id: 0xCAFEBABEDEADBEEF,
        message: vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
    };
    let xdr_bytes = client.to_xdr_bytes().expect("client encode");

    use soroban_sdk::xdr::FromXdr;
    let bytes = Bytes::from_slice(&env, &xdr_bytes);
    let parsed = SharedMessageWithId::from_xdr(&env, &bytes).expect("shared decode");

    assert_eq!(parsed.trigger_id, client.trigger_id);
    let msg_bytes: std::vec::Vec<u8> = parsed.message.iter().collect();
    assert_eq!(msg_bytes, client.message);
}

#[test]
fn message_with_id_shared_to_client_roundtrip() {
    // Shared-encoded XDR bytes must decode into the client `MessageWithId`
    // mirror with the same field values.
    let env = Env::default();
    let trigger_id: u64 = 9_007_199_254_740_993;
    let message: std::vec::Vec<u8> = vec![0x01, 0x02, 0x03, 0xFF];
    let shared = SharedMessageWithId {
        trigger_id,
        message: Bytes::from_slice(&env, &message),
    };

    use soroban_sdk::xdr::ToXdr;
    let xdr_bytes: std::vec::Vec<u8> = shared.to_xdr(&env).iter().collect();

    let parsed = ClientMessageWithId::from_xdr_bytes(&xdr_bytes).expect("client decode");
    assert_eq!(parsed.trigger_id, trigger_id);
    assert_eq!(parsed.message, message);
}

#[test]
fn xlm_envelope_client_to_shared_roundtrip() {
    // Client-encoded XDR bytes must decode into the shared `XlmEnvelope`
    // contracttype with the same field values.
    let env = Env::default();
    let client = ClientXlmEnvelope::new(
        vec![0xAA, 0xBB, 0xCC, 0xDD, 0xEE],
        [0x11; 20],
        [0x22; 12],
    );
    let xdr_bytes = client.encode().expect("client encode");

    use soroban_sdk::xdr::FromXdr;
    let bytes = Bytes::from_slice(&env, &xdr_bytes);
    let parsed = XlmEnvelope::decode(&env, &bytes).expect("shared decode");

    assert_eq!(parsed.event_id.to_array(), client.event_id);
    assert_eq!(parsed.ordering.to_array(), client.ordering);
    let payload_bytes: std::vec::Vec<u8> = parsed.payload.iter().collect();
    assert_eq!(payload_bytes, client.payload);
}

#[test]
fn xlm_envelope_shared_to_client_roundtrip() {
    // Shared-encoded XDR bytes must decode into the client `XlmEnvelope`
    // mirror with the same field values.
    let env = Env::default();
    let event_id = [0x33u8; 20];
    let ordering = [0x44u8; 12];
    let payload: std::vec::Vec<u8> = vec![0x01, 0x02, 0x03, 0xFF];
    let shared = XlmEnvelope {
        event_id: BytesN::from_array(&env, &event_id),
        ordering: BytesN::from_array(&env, &ordering),
        payload: Bytes::from_slice(&env, &payload),
    };

    use soroban_sdk::xdr::ToXdr;
    let xdr_bytes: std::vec::Vec<u8> = shared.to_xdr(&env).iter().collect();

    let parsed = ClientXlmEnvelope::from_xdr_bytes(&xdr_bytes).expect("client decode");
    assert_eq!(parsed.event_id, event_id);
    assert_eq!(parsed.ordering, ordering);
    assert_eq!(parsed.payload, payload);
}

#[test]
fn xlm_envelope_client_decode_rejects_non_map() {
    use wasi_soroban_rs::xdr::WriteXdr;

    let bytes = wasi_soroban_rs::xdr::ScVal::U64(42)
        .to_xdr(wasi_soroban_rs::xdr::Limits::none())
        .unwrap();
    assert!(ClientXlmEnvelope::from_xdr_bytes(&bytes).is_err());
}

#[test]
fn message_with_id_client_decode_rejects_non_map() {
    use wasi_soroban_rs::xdr::WriteXdr;

    let bytes = wasi_soroban_rs::xdr::ScVal::U64(42)
        .to_xdr(wasi_soroban_rs::xdr::Limits::none())
        .unwrap();
    assert!(ClientMessageWithId::from_xdr_bytes(&bytes).is_err());
}

// ── Ed25519: client args + return-value shapes ──────────────────────────

#[test]
fn ed25519_signer_pubkey_decodes_as_bytesn_32() {
    // 32-byte pubkey args travel through `into_val_ext()` (the secp variant uses
    // the same extension trait for [u8; 33]); make sure the 32-byte path lands
    // as a soroban-sdk `BytesN<32>` on the contract side.
    let env = Env::default();
    let client: [u8; 32] = [5u8; 32];
    let sdk_scval = to_sdk(client.into_val_ext().unwrap());

    let parsed: BytesN<32> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn ed25519_signature_bytes_decode_as_bytesn_64() {
    let env = Env::default();
    let client: [u8; 64] = [6u8; 64];
    let sdk_scval = to_sdk(client.into_val_ext().unwrap());

    let parsed: BytesN<64> = try_from_scval(&env, &sdk_scval);
    assert_eq!(parsed.to_array(), client);
}

#[test]
fn ed25519_signer_info_decodes_into_client_struct() {
    // Mirrors the contract->client direction for `list_signers`. The handler
    // returns `Ed25519SignerInfo { key: BytesN<32>, weight: u64 }`; the client
    // must decode that map shape.
    let env = Env::default();
    let key_bytes = [0xABu8; 32];
    let weight: u64 = 12_345;
    let shared = SharedEd25519SignerInfo {
        key: BytesN::from_array(&env, &key_bytes),
        weight,
    };
    let sdk_scval = to_sdk_scval(&env, shared);

    let client_scval = from_sdk(&sdk_scval);
    let parsed = decode_ed25519_signer_info(&client_scval).expect("decode");
    assert_eq!(
        parsed,
        ClientEd25519SignerInfo {
            key: key_bytes,
            weight,
        },
    );
}

#[test]
fn ed25519_signer_info_decode_rejects_wrong_key_size() {
    // Guard against accidentally reusing the secp 33-byte decoder for ed25519
    // (or vice versa): a `SignerInfo` with a 33-byte key must not decode as
    // `Ed25519SignerInfo`.
    let env = Env::default();
    let shared = SharedSignerInfo {
        key: BytesN::from_array(&env, &[0u8; 33]),
        weight: 1,
    };
    let sdk_scval = to_sdk_scval(&env, shared);
    let client_scval = from_sdk(&sdk_scval);
    assert!(decode_ed25519_signer_info(&client_scval).is_err());
}
