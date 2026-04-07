use crate::{Handler, HandlerClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};
use warpdrive_secp256k1_security::Secp256k1Security;
use warpdrive_secp256k1_verification::Secp256k1Verification;

mod contract_wasm {
    use warpdrive_shared::interfaces::CompressedSecpPubKey;

    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_handler.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (HandlerClient<'a>, Address) {
    let admin = Address::generate(env);

    // Deploy security contract
    let security_id = env.register(Secp256k1Security, (&admin, 2u64, 3u64));

    // Deploy verification contract referencing security
    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));

    // Deploy handler contract referencing verification
    let contract_id = env.register(Handler, (&admin, &verification_id));
    let client = HandlerClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
