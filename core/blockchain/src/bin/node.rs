use isa_chain_core::{Blockchain, RpcServer, BlockProducer, Address};
use isa_chain_core::types::constants::MAIN_CHAIN_ID;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    info!("🚀 Starting isA_Chain Node...");

    // Initialize blockchain
    let mut blockchain = Blockchain::new(MAIN_CHAIN_ID);
    info!("✅ Blockchain initialized");

    // Create some test accounts with initial balance
    let test_address1 = Address::new([1u8; 20]);
    let test_address2 = Address::new([2u8; 20]);
    blockchain.mint(test_address1, 1_000_000_000_000_000_000_000); // 1000 ISA
    blockchain.mint(test_address2, 500_000_000_000_000_000_000);   // 500 ISA
    info!("✅ Test accounts created");

    let blockchain = Arc::new(RwLock::new(blockchain));

    // Start block producer in background
    let block_producer = BlockProducer::new(
        blockchain.clone(),
        3, // 3 second block time
        1000, // max 1000 transactions per block
    );
    tokio::spawn(async move {
        block_producer.start().await;
    });
    info!("✅ Block producer started");

    // Start RPC server
    let rpc_port = 9944;
    let rpc_server = RpcServer::new(blockchain.clone(), MAIN_CHAIN_ID, rpc_port);

    println!("
╔══════════════════════════════════════════════════════════╗
║                   isA_Chain Node v0.1.0                  ║
║                                                          ║
║  Your multi-chain compatible blockchain is running!      ║
╚══════════════════════════════════════════════════════════╝

Node Information:
- Chain ID: {}
- Network: Development
- RPC Port: {}
- RPC URL: http://localhost:{}

Test Accounts:
- Account 1: 0x{} (Balance: 1000 ISA)
- Account 2: 0x{} (Balance: 500 ISA)

Available RPC Methods:
- eth_chainId
- eth_blockNumber
- eth_getBalance
- eth_sendRawTransaction
- eth_getTransactionCount
- eth_getBlockByNumber

Status: Running... Press Ctrl+C to stop
",
        MAIN_CHAIN_ID,
        rpc_port,
        rpc_port,
        hex::encode(test_address1.as_bytes()),
        hex::encode(test_address2.as_bytes())
    );

    // Run RPC server
    rpc_server.start().await?;

    Ok(())
}