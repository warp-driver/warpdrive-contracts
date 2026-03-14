use soroban_sdk::{Address, BytesN, Env, Map, String, Vec, contracttype};

// Compressed Ethereum PubKey
pub type PubKey = BytesN<33>;
type SignerMap = Map<PubKey, u64>;

#[contracttype]
pub struct SignerInfo {
    pub key: PubKey,
    pub weight: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    Signers,
}

pub fn get_admin(env: &Env) -> Address {
    env.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(env: &Env, admin: &Address) {
    env.storage().instance().set(&DataKey::Admin, admin);
}

pub fn get_version(env: &Env) -> String {
    env.storage().instance().get(&DataKey::Version).unwrap()
}

pub fn set_version(env: &Env, version: &String) {
    env.storage().instance().set(&DataKey::Version, version);
}

pub fn init_signers(env: &Env) {
    let data = SignerMap::new(env);
    env.storage().instance().set(&DataKey::Signers, &data);
}

pub fn add_signer(env: &Env, key: PubKey, weight: u64) {
    let mut signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    signers.set(key, weight);
    env.storage().instance().set(&DataKey::Signers, &signers);
}

pub fn remove_signer(env: &Env, key: PubKey) {
    let mut signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    signers.remove(key);
    env.storage().instance().set(&DataKey::Signers, &signers);
}

// Sums all the weights of registered signers
pub fn get_total_weight(env: &Env) -> u64 {
    let signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    signers.iter().map(|(_, v)| v).sum()
}

// Returns None if signer is not registered, otherwise their weight
pub fn get_signer_weight(env: &Env, key: PubKey) -> Option<u64> {
    let signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    signers.get(key)
}

// Get all signers, along with weights
pub fn list_signers(env: &Env) -> Vec<SignerInfo> {
    let signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    let mut result = Vec::new(env);
    for (key, weight) in signers.iter() {
        result.push_back(SignerInfo { key, weight });
    }
    result
}
