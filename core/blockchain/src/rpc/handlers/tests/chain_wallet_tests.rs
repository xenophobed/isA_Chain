use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

use crate::blockchain::Blockchain;
use crate::rpc::handlers::RpcHandler;
use crate::rpc::types::*;
use crate::types::Address;
use crate::types::constants::MAIN_CHAIN_ID;
use super::{make_handler, req};

#[tokio::test]
async fn test_chain_get_chain_id() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_CHAIN_ID, json!([])))
        .await;
    assert!(resp.error.is_none());
    assert_eq!(resp.result.unwrap(), json!(MAIN_CHAIN_ID));
}

#[tokio::test]
async fn test_chain_get_height() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_HEIGHT, json!([])))
        .await;
    assert!(resp.error.is_none());
    assert_eq!(resp.result.unwrap(), json!(0u64));
}

#[tokio::test]
async fn test_chain_get_latest_block() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_LATEST_BLOCK, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert!(result.get("hash").is_some());
    assert_eq!(result["height"], json!(0u64));
}

#[tokio::test]
async fn test_chain_get_block_by_height_genesis() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_BLOCK_BY_HEIGHT, json!([0])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["height"], json!(0u64));
}

#[tokio::test]
async fn test_chain_get_block_by_height_not_found() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_BLOCK_BY_HEIGHT, json!([999])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_NOT_FOUND);
}

#[tokio::test]
async fn test_chain_get_block_not_found() {
    let handler = make_handler();
    let fake_hash = hex::encode([0u8; 32]);
    let resp = handler
        .handle_request(req(METHOD_CHAIN_GET_BLOCK, json!([format!("0x{}", fake_hash)])))
        .await;
    assert!(resp.error.is_some());
}

#[tokio::test]
async fn test_wallet_get_balance_zero() {
    let handler = make_handler();
    let addr = format!("0x{}", hex::encode([0xABu8; 20]));
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_BALANCE, json!([addr])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["balance"], json!("0"));
}

#[tokio::test]
async fn test_wallet_get_balance_after_mint() {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    let addr = Address::from([0xABu8; 20]);
    {
        let mut bc = blockchain.write().await;
        bc.mint(addr, 1_000_000);
    }
    let handler = RpcHandler::new(blockchain, MAIN_CHAIN_ID);
    let addr_hex = format!("0x{}", hex::encode(addr.as_bytes()));
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_BALANCE, json!([addr_hex])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["balance"], json!("1000000"));
}

#[tokio::test]
async fn test_wallet_get_nonce_zero() {
    let handler = make_handler();
    let addr = format!("0x{}", hex::encode([0x01u8; 20]));
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_NONCE, json!([addr])))
        .await;
    assert!(resp.error.is_none());
    assert_eq!(resp.result.unwrap(), json!(0u64));
}

#[tokio::test]
async fn test_wallet_get_account_not_found() {
    let handler = make_handler();
    let addr = format!("0x{}", hex::encode([0x99u8; 20]));
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_ACCOUNT, json!([addr])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_NOT_FOUND);
}

#[tokio::test]
async fn test_wallet_get_account_exists() {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    let addr = Address::from([0x42u8; 20]);
    {
        let mut bc = blockchain.write().await;
        bc.mint(addr, 500);
    }
    let handler = RpcHandler::new(blockchain, MAIN_CHAIN_ID);
    let addr_hex = format!("0x{}", hex::encode(addr.as_bytes()));
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_ACCOUNT, json!([addr_hex])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["type"], json!("external"));
    assert_eq!(result["balance"], json!("500"));
}

#[tokio::test]
async fn test_tx_get_pending_empty() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_TX_GET_PENDING, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result, json!([]));
}
