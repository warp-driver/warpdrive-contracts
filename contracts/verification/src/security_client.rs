use soroban_sdk::{BytesN, Env, Vec, contractclient};

#[contractclient(name = "SecurityClient")]
#[allow(dead_code)]
pub trait SecurityInterface {
    fn get_signer_weight(env: Env, key: BytesN<33>) -> u64;
    fn required_weight(env: Env) -> u64;
    fn get_signer_weight_at(env: Env, key: BytesN<33>, reference_block: u32) -> u64;
    fn get_signer_weights(env: Env, keys: Vec<BytesN<33>>) -> Vec<u64>;
    fn get_signer_weights_at(env: Env, keys: Vec<BytesN<33>>, reference_block: u32) -> Vec<u64>;
    fn required_weight_at(env: Env, reference_block: u32) -> u64;
}
