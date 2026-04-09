extern crate alloc;
extern crate std;

use crate::{EthereumHandler, EthereumHandlerClient};

use soroban_sdk::{Address, Env, testutils::Address as _, testutils::Ledger as _};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_secp256k1_verification::Secp256k1Verification;
use warpdrive_shared::testutils::{make_secp256k1_key, secp256k1_pubkey};

use super::handler::{TEST_CURRENT_LEDGER, TEST_REF_BLOCK, make_envelope_bytes_eth, make_sig_data};

#[test]
fn benchmark_max_signatures() {
    let env = Env::default();
    env.mock_all_auths();

    std::println!("\n{:-<60}", "");
    std::println!("Signature capacity benchmark (secp256k1 + EIP-191)");
    std::println!("{:-<60}", "");
    std::println!(
        "{:>4}  {:>14}  {:>12}  {}",
        "N", "CPU", "Memory", "Status"
    );
    std::println!("{:-<60}", "");

    for n in [1u32, 2, 3, 5, 8, 10, 20, 40] {
        // Fresh deploy per iteration to avoid accumulated signers
        let admin = Address::generate(&env);
        env.ledger().set_sequence_number(TEST_REF_BLOCK);

        let security_id = env.register(Secp256k1Security, (&admin, 1u64, 1u64)); // 100% threshold
        let security = Secp256k1SecurityClient::new(&env, &security_id);

        // Register N signers with weight=1 each
        let keys: std::vec::Vec<_> = (1..=n as u8)
            .map(|i| {
                let key = make_secp256k1_key(i);
                let pk = secp256k1_pubkey(&env, &key);
                security.add_signer(&pk, &1u64);
                (key, pk)
            })
            .collect();

        let verification_id = env.register(Secp256k1Verification, (&admin, &security_id));
        let handler_id = env.register(EthereumHandler, (&admin, &verification_id));
        let client = EthereumHandlerClient::new(&env, &handler_id);

        env.ledger().set_sequence_number(TEST_CURRENT_LEDGER);

        // Build envelope with unique event_id per iteration
        let envelope = make_envelope_bytes_eth(&env, n as u8);
        let envelope_raw = envelope.to_alloc_vec();

        // Sign with all N signers, sorted by pubkey
        let sig_data = make_sig_data(&env, &envelope_raw, &keys);

        // Reset to default budget limits, then measure
        env.cost_estimate().budget().reset_default();
        let result = client.try_verify_eth(&envelope, &sig_data);
        let cpu = env.cost_estimate().budget().cpu_instruction_cost();
        let mem = env.cost_estimate().budget().memory_bytes_cost();

        let status = if result.is_ok() { "OK" } else { "EXCEEDED" };
        std::println!("{n:>4}  {cpu:>14}  {mem:>12}  {status}");
    }

    std::println!("{:-<60}", "");
    std::println!("Mainnet limit: 100,000,000 CPU instructions");
    std::println!("{:-<60}\n", "");
}
