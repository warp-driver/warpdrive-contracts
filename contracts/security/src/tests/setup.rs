use crate::{Security, SecurityClient};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env};

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32-unknown-unknown/release/warpdrive_security.wasm"
    );
}

pub fn deploy_contract<'a>(env: &Env) -> (SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(Security, (&admin,));
    let client = SecurityClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
