use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json::{json, Value};

use crate::blockchain::Blockchain;
use crate::rpc::handlers::RpcHandler;
use crate::rpc::types::*;
use crate::staking::StakingVault;
use crate::treasury::ProtocolTreasury;
use crate::types::Address;
use crate::types::constants::MAIN_CHAIN_ID;

pub(crate) fn make_handler() -> RpcHandler {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    RpcHandler::new(blockchain, MAIN_CHAIN_ID)
}

pub(crate) fn make_handler_with_staking() -> RpcHandler {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    let vault = Arc::new(RwLock::new(StakingVault::default_vault()));
    RpcHandler::new(blockchain, MAIN_CHAIN_ID).with_staking(vault)
}

pub(crate) fn make_handler_with_treasury() -> RpcHandler {
    let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
    let treasury = Arc::new(RwLock::new(ProtocolTreasury::new(
        250,
        Address::from([0xAA; 20]),
    )));
    RpcHandler::new(blockchain, MAIN_CHAIN_ID).with_treasury(treasury)
}

pub(crate) fn req(method: &str, params: Value) -> JsonRpcRequest {
    JsonRpcRequest {
        jsonrpc: "2.0".to_string(),
        method: method.to_string(),
        params,
        id: json!(1),
    }
}

mod chain_wallet_tests;
mod ecosystem_tests;
