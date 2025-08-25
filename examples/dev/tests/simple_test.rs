fn wait_for_program_availability(
    program_id: &str,
    endpoint: &str,
    timeout_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("🔄 Test: Waiting for program '{}' to become available via API...", program_id);
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
            },
            Err(_) => {
                if start.elapsed().as_secs() % 5 == 0 {
                    println!("⏳ Test: Still waiting for program availability... ({}/{})", 
                        start.elapsed().as_secs(), timeout_secs);
                }
                std::thread::sleep(std::time::Duration::from_secs(1));
            }
        }
    }
}

#[test]
fn dev() {
    use devtest::{dev, Account};
    use std::str::FromStr;

    const ENDPOINT: &str = "http://localhost:3030";
    let alice: Account<snarkvm::console::network::TestnetV0> =
        Account::from_str("APrivateKey1zkp8CZNn3yeCseEtxuVPbDCwSyhGW6yZKUYKfgXmcpoGPWH").unwrap();

    let dev = dev::new(&alice).unwrap();

    wait_for_program_availability("dev.aleo", ENDPOINT, 60).unwrap();

    let result = dev.main(&alice, 10u32, 5u32).unwrap();
    assert_eq!(result, 15u32);
    println!("✅ Test passed - program is available on network");
}
