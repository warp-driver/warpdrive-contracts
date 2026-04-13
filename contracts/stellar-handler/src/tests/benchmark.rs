extern crate alloc;
extern crate std;

use crate::{StellarHandler, StellarHandlerClient};

use soroban_sdk::{
    Address, Env, testutils::Address as _, testutils::EnvTestConfig, testutils::Ledger as _,
};
use warpdrive_ed25519_security::{Ed25519Security, Ed25519SecurityClient};
use warpdrive_ed25519_verification::Ed25519Verification;
use warpdrive_shared::testutils::{ed25519_pubkey, make_ed25519_key};

use super::handler::{TEST_CURRENT_LEDGER, TEST_REF_BLOCK, make_envelope_bytes_xlm, make_sig_data};

#[test]
fn benchmark_max_signatures() {
    std::println!("\n{:-<80}", "");
    std::println!("Signature capacity benchmark (ed25519 + SEP-0053)");
    std::println!("{:-<80}", "");
    std::println!(
        "{:>4}  {:>14}  {:>12}  {:>10}  {:>10}  Status",
        "N",
        "CPU",
        "Memory",
        "ReadEntry",
        "WriteEntry"
    );
    std::println!("{:-<80}", "");

    for n in [1u32, 2, 3, 5, 8, 10, 20, 40, 60, 80, 90] {
        let result = std::panic::catch_unwind(|| {
            let mut env = Env::default();
            env.mock_all_auths();
            env.set_config(EnvTestConfig {
                capture_snapshot_at_drop: false,
            });

            let admin = Address::generate(&env);
            env.ledger().set_sequence_number(TEST_REF_BLOCK);

            let security_id = env.register(Ed25519Security, (&admin, 1u64, 1u64)); // 100% threshold
            let security = Ed25519SecurityClient::new(&env, &security_id);

            let keys: std::vec::Vec<_> = (1..=n as u8)
                .map(|i| {
                    let key = make_ed25519_key(i);
                    let pk = ed25519_pubkey(&env, &key);
                    security.add_signer(&pk, &1u64);
                    (key, pk)
                })
                .collect();

            let verification_id = env.register(Ed25519Verification, (&admin, &security_id));
            let handler_id = env.register(StellarHandler, (&admin, &verification_id));
            let client = StellarHandlerClient::new(&env, &handler_id);

            env.ledger().set_sequence_number(TEST_CURRENT_LEDGER);

            let envelope = make_envelope_bytes_xlm(&env, n as u8);
            let envelope_raw = envelope.to_alloc_vec();
            let sig_data = make_sig_data(&env, &envelope_raw, &keys);

            env.cost_estimate().budget().reset_unlimited();
            let call_result = client.try_verify_xlm(&envelope, &sig_data);

            let resources = env.cost_estimate().resources();
            (
                call_result.is_ok(),
                resources.instructions,
                resources.mem_bytes,
                resources.memory_read_entries + resources.disk_read_entries,
                resources.write_entries,
            )
        });

        match result {
            Ok((ok, cpu, mem, read_entries, write_entries)) => {
                let status = if !ok {
                    "ERROR"
                } else if cpu > 400_000_000 {
                    "OVER CPU"
                } else if read_entries > 200 {
                    "OVER READS"
                } else {
                    "OK"
                };
                std::println!(
                    "{n:>4}  {cpu:>14}  {mem:>12}  {read_entries:>10}  {write_entries:>10}  {status}"
                );
            }
            Err(_) => {
                std::println!(
                    "{n:>4}  {:>14}  {:>12}  {:>10}  {:>10}  EXCEEDED",
                    "-",
                    "-",
                    "-",
                    "-"
                );
            }
        }
    }

    std::println!("{:-<80}", "");
    std::println!(
        "Mainnet limits: 400M CPU | 200 read entries | 200 write entries | 400 footprint"
    );
    std::println!("{:-<80}\n", "");
}
