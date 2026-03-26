use soroban_sdk::{Address, BytesN, Env, String, Vec, contracttype};
use warpdrive_shared::checkpoint::{self, Checkpoint, CheckpointStore};

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

impl Threshold {
    pub fn new(numerator: u64, denominator: u64) -> Self {
        Threshold {
            numerator,
            denominator,
        }
    }
}

#[contracttype]
#[derive(Clone)]
pub struct StoredCheckpoint {
    pub ledger: u32,
    pub value: u64,
}

#[contracttype]
pub enum DataKey {
    Admin,
    Version,
    Threshold,
    TotalWeight,
    AllSigners,
    Signers(PubKey),
    // Checkpoint keys
    SignerWeightCount(PubKey),
    SignerWeightAt(PubKey, u32),
    TotalWeightCount,
    TotalWeightAt(u32),
}

// ── Admin / Version / Threshold ─────────────────────────────────────

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

// ── Total weight (current snapshot) ─────────────────────────────────

pub fn get_total_weight(env: &Env) -> u64 {
    env.storage()
        .instance()
        .get(&DataKey::TotalWeight)
        .unwrap_or(0u64)
}

fn set_total_weight(env: &Env, weight: u64) {
    env.storage().instance().set(&DataKey::TotalWeight, &weight);
}

// ── Signer management ───────────────────────────────────────────────

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
        .set(&DataKey::Signers(key.clone()), &weight);
    set_total_weight(env, total);

    // Push checkpoints for historical lookups
    checkpoint::push(&SignerWeightHistory::new(env, key), weight);
    checkpoint::push(&TotalWeightHistory::new(env), total);
}

pub fn remove_signer(env: &Env, key: PubKey) {
    if let Some(old_weight) = get_signer_weight(env, key.clone()) {
        let total = get_total_weight(env);
        let new_total = total - old_weight;
        set_total_weight(env, new_total);
        remove_all_signers(env, key.clone());
        env.storage()
            .instance()
            .remove(&DataKey::Signers(key.clone()));

        // Push checkpoints: signer weight drops to 0, total updated
        checkpoint::push(&SignerWeightHistory::new(env, key), 0);
        checkpoint::push(&TotalWeightHistory::new(env), new_total);
    }
}

pub fn get_signer_weight(env: &Env, key: PubKey) -> Option<u64> {
    env.storage().instance().get(&DataKey::Signers(key))
}

// ── Historical lookups ──────────────────────────────────────────────

pub fn get_signer_weight_at(env: &Env, key: PubKey, reference_block: u32) -> u64 {
    checkpoint::lookup_at(&SignerWeightHistory::new(env, key), reference_block)
}

pub fn get_signer_weights(env: &Env, keys: &Vec<PubKey>) -> Vec<u64> {
    let mut result = Vec::new(env);
    for i in 0..keys.len() {
        let key = keys.get(i).unwrap();
        result.push_back(get_signer_weight(env, key).unwrap_or(0));
    }
    result
}

pub fn get_signer_weights_at(env: &Env, keys: &Vec<PubKey>, reference_block: u32) -> Vec<u64> {
    let mut result = Vec::new(env);
    for i in 0..keys.len() {
        let key = keys.get(i).unwrap();
        result.push_back(get_signer_weight_at(env, key, reference_block));
    }
    result
}

pub fn get_total_weight_at(env: &Env, reference_block: u32) -> u64 {
    checkpoint::lookup_at(&TotalWeightHistory::new(env), reference_block)
}

// ── Signer enumeration (for UI) ─────────────────────────────────────

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
            core::cmp::Ordering::Equal => return,
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
            break;
        }
    }
}

// ── CheckpointStore implementations ─────────────────────────────────

pub struct SignerWeightHistory<'a> {
    env: &'a Env,
    key: PubKey,
}

impl<'a> SignerWeightHistory<'a> {
    pub fn new(env: &'a Env, key: PubKey) -> Self {
        Self { env, key }
    }
}

impl CheckpointStore for SignerWeightHistory<'_> {
    type Value = u64;

    fn count(&self) -> u32 {
        self.env
            .storage()
            .instance()
            .get(&DataKey::SignerWeightCount(self.key.clone()))
            .unwrap_or(0u32)
    }

    fn set_count(&self, count: u32) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::SignerWeightCount(self.key.clone()), &count);
    }

    fn get(&self, index: u32) -> Checkpoint<u64> {
        let sc: StoredCheckpoint = self
            .env
            .storage()
            .instance()
            .get(&DataKey::SignerWeightAt(self.key.clone(), index))
            .unwrap();
        Checkpoint {
            ledger: sc.ledger,
            value: sc.value,
        }
    }

    fn set(&self, index: u32, cp: Checkpoint<u64>) {
        let sc = StoredCheckpoint {
            ledger: cp.ledger,
            value: cp.value,
        };
        self.env
            .storage()
            .instance()
            .set(&DataKey::SignerWeightAt(self.key.clone(), index), &sc);
    }

    fn current_ledger(&self) -> u32 {
        self.env.ledger().sequence()
    }
}

pub struct TotalWeightHistory<'a> {
    env: &'a Env,
}

impl<'a> TotalWeightHistory<'a> {
    pub fn new(env: &'a Env) -> Self {
        Self { env }
    }
}

impl CheckpointStore for TotalWeightHistory<'_> {
    type Value = u64;

    fn count(&self) -> u32 {
        self.env
            .storage()
            .instance()
            .get(&DataKey::TotalWeightCount)
            .unwrap_or(0u32)
    }

    fn set_count(&self, count: u32) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::TotalWeightCount, &count);
    }

    fn get(&self, index: u32) -> Checkpoint<u64> {
        let sc: StoredCheckpoint = self
            .env
            .storage()
            .instance()
            .get(&DataKey::TotalWeightAt(index))
            .unwrap();
        Checkpoint {
            ledger: sc.ledger,
            value: sc.value,
        }
    }

    fn set(&self, index: u32, cp: Checkpoint<u64>) {
        let sc = StoredCheckpoint {
            ledger: cp.ledger,
            value: cp.value,
        };
        self.env
            .storage()
            .instance()
            .set(&DataKey::TotalWeightAt(index), &sc);
    }

    fn current_ledger(&self) -> u32 {
        self.env.ledger().sequence()
    }
}
