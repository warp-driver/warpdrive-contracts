use crate::{Security, SecurityClient};
use soroban_sdk::{Address, BytesN, Env, testutils::Address as _};

mod contract_wasm {
    soroban_sdk::contractimport!(
        file = "../../target/wasm32v1-none/release/warpdrive_security.wasm"
    );
}

// Set it up with 2/3 threshold
pub const THRESHOLD_NUM: u64 = 2u64;
pub const THRESHOLD_DENOM: u64 = 3u64;

pub fn deploy_contract<'a>(env: &Env) -> (SecurityClient<'a>, Address) {
    let admin = Address::generate(env);
    let contract_id = env.register(Security, (&admin, THRESHOLD_NUM, THRESHOLD_DENOM));
    let client = SecurityClient::new(env, &contract_id);
    (client, admin)
}

pub fn install_contract_wasm(env: &Env) -> BytesN<32> {
    env.deployer().upload_contract_wasm(contract_wasm::WASM)
}
