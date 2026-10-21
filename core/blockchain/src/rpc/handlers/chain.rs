use serde_json::{json, Value};
use tracing::{info, warn};
use crate::rpc::types::*;
use super::helpers::{block_to_json, parse_hash_param};
use super::RpcHandler;

impl RpcHandler {
    /// chain_getBlock(hash) — get block by hash
    pub(crate) async fn handle_chain_get_block(&self, id: Value, params: Value) -> JsonRpcResponse {
        let hash = match parse_hash_param(&params, 0) {
            Ok(h) => h,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };

        let blockchain = self.blockchain.read().await;
        match blockchain.get_block(&hash) {
            Some(block) => {
                info!("chain_getBlock: found block at height={}", block.header.height);
                JsonRpcResponse::success(id, block_to_json(block))
            }
            None => {
                warn!("chain_getBlock: block not found: 0x{}", hex::encode(hash.as_bytes()));
                JsonRpcResponse::error(
                    id,
                    ERROR_NOT_FOUND,
                    format!("Block not found: 0x{}", hex::encode(hash.as_bytes())),
                )
            }
        }
    }

    /// chain_getBlockByHeight(height) — get block by height
    pub(crate) async fn handle_chain_get_block_by_height(&self, id: Value, params: Value) -> JsonRpcResponse {
        let arr = match params.as_array() {
            Some(a) => a,
            None => {
                return JsonRpcResponse::error(id, ERROR_INVALID_PARAMS, "params must be an array".to_string())
            }
        };

        let height: u64 = match arr.first() {
            Some(v) => {
                if let Some(n) = v.as_u64() {
                    n
                } else if let Some(s) = v.as_str() {
                    let s = s.trim_start_matches("0x");
                    match u64::from_str_radix(s, 16) {
                        Ok(n) => n,
                        Err(_) => match s.parse::<u64>() {
                            Ok(n) => n,
                            Err(_) => {
                                return JsonRpcResponse::error(
                                    id,
                                    ERROR_INVALID_PARAMS,
                                    "Invalid block height".to_string(),
                                )
                            }
                        },
                    }
                } else {
                    return JsonRpcResponse::error(
                        id,
                        ERROR_INVALID_PARAMS,
                        "height must be a number or hex string".to_string(),
                    );
                }
            }
            None => {
                return JsonRpcResponse::error(
                    id,
                    ERROR_INVALID_PARAMS,
                    "Missing height parameter".to_string(),
                )
            }
        };

        let blockchain = self.blockchain.read().await;
        match blockchain.get_block_by_height(height) {
            Some(block) => {
                info!("chain_getBlockByHeight: found block at height={}", block.header.height);
                JsonRpcResponse::success(id, block_to_json(block))
            }
            None => {
                warn!("chain_getBlockByHeight: block not found at height={}", height);
                JsonRpcResponse::error(
                    id,
                    ERROR_NOT_FOUND,
                    format!("Block not found at height {}", height),
                )
            }
        }
    }

    /// chain_getLatestBlock() — get latest block
    pub(crate) async fn handle_chain_get_latest_block(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        match blockchain.get_latest_block() {
            Some(block) => {
                info!("chain_getLatestBlock: height={}", block.header.height);
                JsonRpcResponse::success(id, block_to_json(block))
            }
            None => {
                warn!("chain_getLatestBlock: no blocks in chain");
                JsonRpcResponse::error(id, ERROR_INTERNAL, "No blocks in chain".to_string())
            }
        }
    }

    /// chain_getHeight() — current chain height
    pub(crate) async fn handle_chain_get_height(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        JsonRpcResponse::success(id, json!(blockchain.get_height()))
    }

    /// chain_getChainId() — chain ID
    pub(crate) async fn handle_chain_get_chain_id(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(id, json!(self.chain_id))
    }
}
