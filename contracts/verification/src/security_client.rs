use soroban_sdk::{BytesN, Env, contractclient};

#[contractclient(name = "SecurityClient")]
#[allow(dead_code)]
pub trait SecurityInterface {
    fn get_signer_weight(env: Env, key: BytesN<33>) -> u64;
    fn required_weight(env: Env) -> u64;
}
