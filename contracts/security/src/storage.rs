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
#[derive(Clone)]
pub struct Threshold {
    pub numerator: u64,
    pub denominator: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    Signers,
    Threshold,
    TotalWeight,
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

pub fn get_threshold(env: &Env) -> Threshold {
    env.storage().instance().get(&DataKey::Threshold).unwrap()
}

pub fn set_threshold(env: &Env, threshold: &Threshold) {
    env.storage().instance().set(&DataKey::Threshold, threshold);
}

pub fn init_signers(env: &Env) {
    let data = SignerMap::new(env);
    env.storage().instance().set(&DataKey::Signers, &data);
    env.storage().instance().set(&DataKey::TotalWeight, &0u64);
}

pub fn get_total_weight(env: &Env) -> u64 {
    env.storage().instance().get(&DataKey::TotalWeight).unwrap()
}

fn set_total_weight(env: &Env, weight: u64) {
    env.storage().instance().set(&DataKey::TotalWeight, &weight);
}

pub fn add_signer(env: &Env, key: PubKey, weight: u64) {
    let mut signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();
    let mut total = get_total_weight(env);

    // If updating an existing signer, subtract the old weight first
    if let Some(old_weight) = signers.get(key.clone()) {
        total -= old_weight;
    }

    total = total
        .checked_add(weight)
        .expect("total weight would overflow u64");

    signers.set(key, weight);
    env.storage().instance().set(&DataKey::Signers, &signers);
    set_total_weight(env, total);
}

pub fn remove_signer(env: &Env, key: PubKey) {
    let mut signers: SignerMap = env.storage().instance().get(&DataKey::Signers).unwrap();

    // It it wasn't in the map before, we simplify this to a no-op
    // QUESTION: should we return an error on the else branch?
    if let Some(old_weight) = signers.get(key.clone()) {
        let total = get_total_weight(env);
        set_total_weight(env, total - old_weight);
        signers.remove(key);
        env.storage().instance().set(&DataKey::Signers, &signers);
    }
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
