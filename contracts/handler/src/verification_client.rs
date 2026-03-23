use soroban_sdk::{Bytes, BytesN, Env, Vec, contractclient};

type PubKey = BytesN<33>;

#[contractclient(name = "VerificationClient")]
#[allow(dead_code)]
pub trait VerificationInterface {
    fn verify(env: Env, envelope: Bytes, signatures: Vec<BytesN<65>>, signer_pubkeys: Vec<PubKey>);
}
