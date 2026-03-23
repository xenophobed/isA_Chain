use isa_chain_core::{Transaction, TransactionData, Address};
use isa_chain_core::types::constants::{MAIN_CHAIN_ID, BASE_GAS_PRICE};
use reqwest;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 Creating and sending a test transaction...\n");

    // Test account addresses
    let from_address = Address::new([1u8; 20]);
    let to_address = Address::new([2u8; 20]);

    println!("From: 0x{}", hex::encode(from_address.as_bytes()));
    println!("To: 0x{}", hex::encode(to_address.as_bytes()));

    // Get current nonce
    let client = reqwest::Client::new();
    let nonce_response = client
        .post("http://localhost:9944")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionCount",
            "params": [format!("0x{}", hex::encode(from_address.as_bytes())), "latest"],
            "id": 1
        }))
        .send()
        .await?;

    let nonce_result: serde_json::Value = nonce_response.json().await?;
    let nonce_hex = nonce_result["result"].as_str().unwrap_or("0x0");
    let nonce = u64::from_str_radix(nonce_hex.trim_start_matches("0x"), 16)?;

    println!("Current nonce: {}", nonce);

    // Create transaction
    let mut tx = Transaction::new(
        from_address,
        nonce,
        TransactionData::Transfer {
            to: to_address,
            amount: 100_000_000_000_000_000_000, // 100 ISA
            data: vec![],
        },
        21000, // gas_limit
        BASE_GAS_PRICE,
        MAIN_CHAIN_ID,
    );

    println!("\n📝 Transaction created:");
    println!("  Amount: 100 ISA");
    println!("  Gas Limit: 21000");
    println!("  Gas Price: {}", BASE_GAS_PRICE);

    // Sign transaction with test private key
    let private_key = [1u8; 32]; // Test private key
    tx.sign(&private_key)?;

    println!("\n✍️  Transaction signed");

    // Serialize transaction
    let tx_bytes = bincode::serialize(&tx)?;
    let tx_hex = format!("0x{}", hex::encode(&tx_bytes));

    println!("  Serialized size: {} bytes", tx_bytes.len());

    // Send transaction via RPC
    println!("\n📤 Sending transaction to RPC...");

    let send_response = client
        .post("http://localhost:9944")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_sendRawTransaction",
            "params": [tx_hex],
            "id": 2
        }))
        .send()
        .await?;

    let send_result: serde_json::Value = send_response.json().await?;

    if let Some(error) = send_result.get("error") {
        println!("❌ Transaction failed: {}", error);
        return Ok(());
    }

    let tx_hash = send_result["result"].as_str().unwrap_or("unknown");
    println!("✅ Transaction accepted!");
    println!("  Transaction hash: {}", tx_hash);

    // Wait a bit for block to be produced
    println!("\n⏳ Waiting for block production (3 seconds)...");
    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;

    // Check block number
    let block_response = client
        .post("http://localhost:9944")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_blockNumber",
            "params": [],
            "id": 3
        }))
        .send()
        .await?;

    let block_result: serde_json::Value = block_response.json().await?;
    let block_number_hex = block_result["result"].as_str().unwrap_or("0x0");
    let block_number = u64::from_str_radix(block_number_hex.trim_start_matches("0x"), 16)?;

    println!("\n📊 Current block height: {}", block_number);

    // Check balances
    let from_balance_response = client
        .post("http://localhost:9944")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [format!("0x{}", hex::encode(from_address.as_bytes())), "latest"],
            "id": 4
        }))
        .send()
        .await?;

    let from_balance_result: serde_json::Value = from_balance_response.json().await?;
    let from_balance_hex = from_balance_result["result"].as_str().unwrap_or("0x0");
    let from_balance = u128::from_str_radix(from_balance_hex.trim_start_matches("0x"), 16)?;

    let to_balance_response = client
        .post("http://localhost:9944")
        .json(&json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [format!("0x{}", hex::encode(to_address.as_bytes())), "latest"],
            "id": 5
        }))
        .send()
        .await?;

    let to_balance_result: serde_json::Value = to_balance_response.json().await?;
    let to_balance_hex = to_balance_result["result"].as_str().unwrap_or("0x0");
    let to_balance = u128::from_str_radix(to_balance_hex.trim_start_matches("0x"), 16)?;

    println!("\n💰 Account balances:");
    println!("  From: {} ISA", from_balance / 1_000_000_000_000_000_000);
    println!("  To:   {} ISA", to_balance / 1_000_000_000_000_000_000);

    println!("\n✅ Test complete!");

    Ok(())
}
