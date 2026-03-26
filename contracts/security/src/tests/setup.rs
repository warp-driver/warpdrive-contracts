use crate::{Security, SecurityClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};

mod contract_wasm {
    use warpdrive_shared::interfaces::PubKey;

    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_security.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(Security, (&admin, 2u64, 3u64));
    let client = SecurityClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
