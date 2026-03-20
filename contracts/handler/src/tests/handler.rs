extern crate std;

use crate::envelope::Envelope;
use crate::{Handler, HandlerClient, HandlerError, SignatureData};
use alloy_primitives::FixedBytes;
use alloy_sol_types::SolValue;
use soroban_sdk::{Bytes, Env, Vec, testutils::Address as _};
use warpdrive_security::Security;
use warpdrive_verification::Verification;

fn make_envelope_bytes(env: &Env, event_id_seed: u8) -> Bytes {
    let mut event_id = [0u8; 20];
    event_id[0] = event_id_seed;

    let envelope = Envelope {
        eventId: FixedBytes(event_id),
        ordering: FixedBytes([0u8; 12]),
        payload: alloc::vec![].into(),
    };

    let encoded = envelope.abi_encode();
    Bytes::from_slice(env, &encoded)
}

fn empty_sig_data(env: &Env) -> SignatureData {
    SignatureData {
        signers: Vec::new(env),
        signatures: Vec::new(env),
        reference_block: 0,
    }
}

fn setup_handler<'a>(env: &Env) -> HandlerClient<'a> {
    let admin = soroban_sdk::Address::generate(env);

    let security_id = env.register(Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Verification, (&admin, &security_id));
    let handler_id = env.register(Handler, (&admin, &verification_id));

    HandlerClient::new(env, &handler_id)
}

#[test]
fn test_verify_first_event_succeeds() {
    let env = Env::default();
    let client = setup_handler(&env);

    let envelope = make_envelope_bytes(&env, 1);
    let sig_data = empty_sig_data(&env);

    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));
}

#[test]
fn test_verify_duplicate_event_fails() {
    let env = Env::default();
    let client = setup_handler(&env);

    let envelope = make_envelope_bytes(&env, 1);
    let sig_data = empty_sig_data(&env);

    // First call succeeds
    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Ok(Ok(())));

    // Second call with same event_id fails
    let result = client.try_verify(&envelope, &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));
}

#[test]
fn test_verify_different_events_succeed() {
    let env = Env::default();
    let client = setup_handler(&env);
    let sig_data = empty_sig_data(&env);

    // Event 1 succeeds
    let result = client.try_verify(&make_envelope_bytes(&env, 1), &sig_data);
    assert_eq!(result, Ok(Ok(())));

    // Event 1 again fails
    let result = client.try_verify(&make_envelope_bytes(&env, 1), &sig_data);
    assert_eq!(result, Err(Ok(HandlerError::EventAlreadySeen)));

    // Event 2 succeeds
    let result = client.try_verify(&make_envelope_bytes(&env, 2), &sig_data);
    assert_eq!(result, Ok(Ok(())));
}

extern crate alloc;
