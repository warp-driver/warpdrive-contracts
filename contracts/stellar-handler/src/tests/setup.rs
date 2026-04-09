use crate::{StellarHandler, StellarHandlerClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};
use warpdrive_ed25519_security::Ed25519Security;
use warpdrive_ed25519_verification::Ed25519Verification;

mod contract_wasm {
    use warpdrive_shared::interfaces::{CompressedSecpPubKey, Ed25519PubKey};

    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_stellar_handler.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (StellarHandlerClient<'a>, Address) {
    let admin = Address::generate(env);

    let security_id = env.register(Ed25519Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Ed25519Verification, (&admin, &security_id));
    let contract_id = env.register(StellarHandler, (&admin, &verification_id));
    let client = StellarHandlerClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
