use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::json;

use crate::blockchain::Blockchain;
use crate::rpc::handlers::RpcHandler;
use crate::rpc::types::*;
use crate::staking::StakingVault;
use crate::types::Address;
use crate::types::constants::{INITIAL_SUPPLY, MAIN_CHAIN_ID, VALIDATOR_MIN_STAKE};
use super::{make_handler, make_handler_with_staking, make_handler_with_treasury, req};

#[tokio::test]
async fn test_token_get_supply() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_TOKEN_GET_SUPPLY, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["total_supply"], json!(INITIAL_SUPPLY.to_string()));
    assert_eq!(result["total_burned"], json!("0"));
}

#[tokio::test]
async fn test_token_get_price_no_oracle() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_TOKEN_GET_PRICE, json!([])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_NOT_FOUND);
}

#[tokio::test]
async fn test_staking_get_total_staked_no_vault() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_STAKING_GET_TOTAL_STAKED, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["total_staked"], json!("0"));
}

#[tokio::test]
async fn test_staking_get_stake_not_found() {
    let handler = make_handler_with_staking();
    let addr = format!("0x{}", hex::encode([0x01u8; 20]));
    let resp = handler
        .handle_request(req(METHOD_STAKING_GET_STAKE, json!([addr])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_NOT_FOUND);
}

#[tokio::test]
async fn test_staking_get_stake_exists() {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    let vault = Arc::new(RwLock::new(StakingVault::default_vault()));
    let staker = Address::from([0x55u8; 20]);
    {
        let mut v = vault.write().await;
        v.stake(staker, VALIDATOR_MIN_STAKE, 1).unwrap();
    }
    let handler =
        RpcHandler::new(blockchain, MAIN_CHAIN_ID).with_staking(vault);
    let addr_hex = format!("0x{}", hex::encode(staker.as_bytes()));
    let resp = handler
        .handle_request(req(METHOD_STAKING_GET_STAKE, json!([addr_hex])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["staked"], json!(VALIDATOR_MIN_STAKE.to_string()));
}

#[tokio::test]
async fn test_treasury_get_balance_no_treasury() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_TREASURY_GET_BALANCE, json!([])))
        .await;
    assert!(resp.error.is_none());
    assert_eq!(resp.result.unwrap()["balance"], json!("0"));
}

#[tokio::test]
async fn test_treasury_get_stats_with_treasury() {
    let handler = make_handler_with_treasury();
    let resp = handler
        .handle_request(req(METHOD_TREASURY_GET_STATS, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["balance"], json!("0"));
    assert_eq!(result["fee_rate_bps"], json!(250u32));
}

#[tokio::test]
async fn test_invalid_address_returns_error() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_WALLET_GET_BALANCE, json!(["not-a-valid-address"])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_INVALID_PARAMS);
}

#[tokio::test]
async fn test_unknown_method_returns_not_found() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req("made_up_method", json!([])))
        .await;
    assert!(resp.error.is_some());
    assert_eq!(resp.error.unwrap().code, ERROR_METHOD_NOT_FOUND);
}

#[tokio::test]
async fn test_system_health() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_SYSTEM_HEALTH, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["status"], json!("ok"));
    assert_eq!(result["height"], json!(0u64));
    assert_eq!(result["peers"], json!(0u64));
    assert_eq!(result["syncing"], json!(false));
}

#[tokio::test]
async fn test_system_version() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_SYSTEM_VERSION, json!([])))
        .await;
    assert!(resp.error.is_none());
    let result = resp.result.unwrap();
    assert_eq!(result["version"], json!("0.1.0"));
    assert_eq!(result["chain_id"], json!(MAIN_CHAIN_ID));
}

#[tokio::test]
async fn test_system_metrics_rpc_returns_prometheus_text() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_SYSTEM_METRICS, json!([])))
        .await;
    assert!(resp.error.is_none());

    let text = resp.result.unwrap();
    let text = text.as_str().expect("system_metrics result should be a string");

    assert!(text.contains("isa_chain_height"), "missing isa_chain_height");
    assert!(text.contains("isa_chain_mempool_size"), "missing isa_chain_mempool_size");
    assert!(text.contains("isa_chain_rpc_requests_total"), "missing rpc counter");
    assert!(text.contains("isa_chain_rpc_requests_total 1"), "counter should be 1 after first request");
}

#[tokio::test]
async fn test_system_metrics_rpc_counter_increments() {
    let handler = make_handler();

    for _ in 0..3 {
        handler
            .handle_request(req(METHOD_SYSTEM_HEALTH, json!([])))
            .await;
    }

    let resp = handler
        .handle_request(req(METHOD_SYSTEM_METRICS, json!([])))
        .await;
    assert!(resp.error.is_none());

    let text = resp.result.unwrap();
    let text = text.as_str().unwrap();
    assert!(text.contains("isa_chain_rpc_requests_total 4"), "expected 4 rpc requests, got:\n{text}");
}

#[tokio::test]
async fn test_system_metrics_rpc_format_valid() {
    let handler = make_handler();
    let resp = handler
        .handle_request(req(METHOD_SYSTEM_METRICS, json!([])))
        .await;
    let text = resp.result.unwrap();
    let text = text.as_str().unwrap();

    for line in text.lines() {
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        assert_eq!(parts.len(), 2, "bad prometheus line: {line}");
        parts[1].parse::<u64>().expect("metric value must be u64");
    }
}
