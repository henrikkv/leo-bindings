fn wait_for_program_availability(
    program_id: &str,
    endpoint: &str,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!(
        "🔄 Test: Waiting for program '{}' to become available via API...",
        program_id
    );
    let start = std::time::Instant::now();

    loop {
        if start.elapsed().as_secs() > timeout_secs {
            return Err(format!("Timeout waiting for program {}", program_id).into());
        }

        let response = ureq::get(&format!("{}/testnet/program/{}", endpoint, program_id)).call();
        match response {
            Ok(_) => {
                println!("✅ Test: Program '{}' is now available via API", program_id);
                return Ok(());
            }
            Err(_) => {
                if start.elapsed().as_secs() % 5 == 0 {
                    println!(
                        "⏳ Test: Still waiting for program availability... ({}/{})",
                        start.elapsed().as_secs(),
                        timeout_secs
                    );
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

#[test]
fn token() {
    use leo_bindings::utils::Account;
    use std::str::FromStr;
    use tokenexample::*;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<snarkvm::console::network::TestnetV0> =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();

    let token = token::new(&alice, ENDPOINT).unwrap();

    wait_for_program_availability("token.aleo", ENDPOINT, 60).unwrap();

    let rec = token.mint_private(&alice, alice.address(), 100).unwrap();
    dbg!(&rec);
    let (rec1, rec2) = token
        .transfer_private(&alice, rec, alice.address(), 10)
        .unwrap();
    dbg!(&rec1);
    dbg!(&rec2);
}
