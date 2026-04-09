extern crate alloc;
extern crate std;

use crate::{EthereumHandler, EthereumHandlerClient};

use soroban_sdk::{Address, Env, testutils::{Address as _, EnvTestConfig, Ledger as _}};
use warpdrive_secp256k1_security::{Secp256k1Security, Secp256k1SecurityClient};
use warpdrive_secp256k1_verification::Secp256k1Verification;
use warpdrive_shared::testutils::{make_secp256k1_key, secp256k1_pubkey};

use super::handler::{TEST_CURRENT_LEDGER, TEST_REF_BLOCK, make_envelope_bytes_eth, make_sig_data};

#[test]
fn benchmark_max_signatures() {
    std::println!("\n{:-<80}", "");
    std::println!("Signature capacity benchmark (secp256k1 + EIP-191)");
    std::println!("{:-<80}", "");
    std::println!(
        "{:>4}  {:>14}  {:>12}  {:>10}  {:>10}  {}",
        "N", "CPU", "Memory", "ReadEntry", "WriteEntry", "Status"
    );
    std::println!("{:-<80}", "");

    for n in [1u32, 2, 3, 5, 8, 10, 20, 40, 60, 80, 90, 100] {
        let result = std::panic::catch_unwind(|| {
            let mut env = Env::default();
            env.mock_all_auths();
            env.set_config(EnvTestConfig {
                capture_snapshot_at_drop: false,
            });

            let admin = Address::generate(&env);
            env.ledger().set_sequence_number(TEST_REF_BLOCK);

            let security_id = env.register(Secp256k1Security, (&admin, 1u64, 1u64)); // 100% threshold
            let security = Secp256k1SecurityClient::new(&env, &security_id);

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

            let envelope = make_envelope_bytes_eth(&env, n as u8);
            let envelope_raw = envelope.to_alloc_vec();
            let sig_data = make_sig_data(&env, &envelope_raw, &keys);

            env.cost_estimate().budget().reset_unlimited();
            let call_result = client.try_verify_eth(&envelope, &sig_data);

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
                    "-", "-", "-", "-"
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
