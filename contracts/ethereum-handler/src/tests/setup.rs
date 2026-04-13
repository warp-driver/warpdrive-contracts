use crate::{EthereumHandler, EthereumHandlerClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};
use warpdrive_secp256k1_security::Secp256k1Security;
use warpdrive_secp256k1_verification::Secp256k1Verification;

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_ethereum_handler.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (EthereumHandlerClient<'a>, Address) {
    let admin = Address::generate(env);

    let security_id = env.register(Secp256k1Security, (&admin, 2u64, 3u64));
    let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
    let contract_id = env.register(EthereumHandler, (&admin, &verification_id));
    let client = EthereumHandlerClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
