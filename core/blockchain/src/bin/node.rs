use isa_chain_core::{Blockchain, RpcServer, BlockProducer};
use isa_chain_core::consensus::ConsensusEngine;
use isa_chain_core::storage::RocksDbStorage;
use isa_chain_core::types::constants::MAIN_CHAIN_ID;
use std::sync::Arc;
use std::env;
use tokio::sync::RwLock;
use tracing::info;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("isa_chain=info".parse()?)
        )
        .init();

    // Configuration from environment
    let chain_id = env::var("CHAIN_ID")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(MAIN_CHAIN_ID);
    let rpc_port = env::var("RPC_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(9944u16);
    let block_time = env::var("BLOCK_TIME_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(3u64);
    let data_dir = env::var("DATA_DIR")
        .unwrap_or_else(|_| "./data".to_string());

    info!("Starting isA_Chain node");
    info!("  Chain ID: {}", chain_id);
    info!("  RPC Port: {}", rpc_port);
    info!("  Block Time: {}s", block_time);
    info!("  Data Dir: {}", data_dir);

    // Initialize blockchain with optional persistence
    let blockchain = if env::var("PERSIST").unwrap_or_default() == "true" {
        std::fs::create_dir_all(&data_dir)?;
        let storage = RocksDbStorage::new(&data_dir)?;
        Blockchain::new_with_storage(chain_id, storage)?
    } else {
        Blockchain::new(chain_id)
    };

    let blockchain = Arc::new(RwLock::new(blockchain));

    // Start block producer
    let consensus = Arc::new(RwLock::new(ConsensusEngine::new()));
    let block_producer = BlockProducer::new(
        blockchain.clone(),
        consensus,
        block_time,
        100,
    );
    tokio::spawn(async move {
        block_producer.start().await;
    });

    // Start RPC server (blocks)
    info!("RPC server listening on 0.0.0.0:{}", rpc_port);
    let rpc = RpcServer::new(blockchain, chain_id, rpc_port);
    rpc.start().await?;

    Ok(())
}
