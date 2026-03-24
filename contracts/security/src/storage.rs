use soroban_sdk::{Address, BytesN, Env, String, Vec, contracttype};

// Compressed Ethereum PubKey
pub type PubKey = BytesN<33>;

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
    Threshold,
    TotalWeight,
    AllSigners,
    Signers(PubKey),
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

pub fn get_total_weight(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::TotalWeight)
        .unwrap_or(0u64)
}

fn set_total_weight(env: &Env, weight: u64) {
    env.storage().instance().set(&DataKey::TotalWeight, &weight);
}

pub fn add_signer(env: &Env, key: PubKey, weight: u64) {
    let mut total = get_total_weight(env);

    // If updating an existing signer, subtract the old weight first
    if let Some(old_weight) = get_signer_weight(env, key.clone()) {
        total -= old_weight;
    } else {
        insert_all_signers(env, key.clone());
    }

    total = total
        .checked_add(weight)
        .expect("total weight would overflow u64");

    env.storage()
        .instance()
        .set(&DataKey::Signers(key), &weight);
    set_total_weight(env, total);
}

pub fn remove_signer(env: &Env, key: PubKey) {
    // It it wasn't in the map before, we simplify this to a no-op
    // QUESTION: should we return an error on the else branch?
    if let Some(old_weight) = get_signer_weight(env, key.clone()) {
        let total = get_total_weight(env);
        set_total_weight(env, total - old_weight);
        remove_all_signers(env, key.clone());
        env.storage().instance().remove(&DataKey::Signers(key));
    }
}

// Returns None if signer is not registered, otherwise their weight
pub fn get_signer_weight(env: &Env, key: PubKey) -> Option<u64> {
    env.storage().instance().get(&DataKey::Signers(key))
}

// Get all signers, along with weights. This should be an infrequently called query and as such is not optimized
pub fn list_signers(env: &Env) -> Vec<SignerInfo> {
    let signers = all_signers(env);
    let mut result = Vec::new(env);
    for key in signers.into_iter() {
        if let Some(weight) = get_signer_weight(env, key.clone()) {
            result.push_back(SignerInfo { key, weight });
        }
    }
    result
}

fn all_signers(env: &Env) -> Vec<PubKey> {
    env.storage()
        .instance()
        .get(&DataKey::AllSigners)
        .unwrap_or_else(|| Vec::new(env))
}

fn set_all_signers(env: &Env, signers: &Vec<PubKey>) {
    env.storage().instance().set(&DataKey::AllSigners, signers)
}

// Ensure the AllSigners array is always sorted properly in ascending pubkey order
fn insert_all_signers(env: &Env, key: PubKey) {
    let mut signers = all_signers(env);
    let len = signers.len();
    let mut idx = len;
    for i in 0..len {
        let existing = signers.get(i).unwrap();
        match key.cmp(&existing) {
            core::cmp::Ordering::Less => {
                idx = i;
                break;
            }
            core::cmp::Ordering::Equal => return, // already present, no-op
            core::cmp::Ordering::Greater => {}
        }
    }
    signers.insert(idx, key);
    set_all_signers(env, &signers);
}

fn remove_all_signers(env: &Env, key: PubKey) {
    let mut signers = all_signers(env);
    for i in 0..signers.len() {
        let existing = signers.get(i).unwrap();
        if existing == key {
            signers.remove(i);
            set_all_signers(env, &signers);
            return;
        }
        if existing > key {
            break; // since sorted, key can't appear later
        }
    }
    // key not found — no-op
}
