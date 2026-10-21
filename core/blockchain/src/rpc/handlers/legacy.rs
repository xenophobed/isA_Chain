use serde_json::{json, Value};
use tracing::{info, warn};
use crate::rpc::types::*;
use super::helpers::parse_address_param;
use super::RpcHandler;

impl RpcHandler {
    pub(crate) async fn handle_chain_id(&self, id: Value) -> JsonRpcResponse {
        let chain_id_hex = format!("0x{:x}", self.chain_id);
        JsonRpcResponse::success(id, json!(chain_id_hex))
    }

    pub(crate) async fn handle_block_number(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let block_number_hex = format!("0x{:x}", blockchain.get_height());
        JsonRpcResponse::success(id, json!(block_number_hex))
    }

    pub(crate) async fn handle_get_balance(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };
        let blockchain = self.blockchain.read().await;
        let balance_hex = format!("0x{:x}", blockchain.get_balance(&address));
        JsonRpcResponse::success(id, json!(balance_hex))
    }

    pub(crate) async fn handle_send_raw_transaction(&self, id: Value, params: Value) -> JsonRpcResponse {
        let arr = match params.as_array() {
            Some(a) => a,
            None => {
                return JsonRpcResponse::error(id, ERROR_INVALID_PARAMS, "Invalid params".to_string())
            }
        };

        let tx_data_str = match arr.first().and_then(|v| v.as_str()) {
            Some(s) => s,
            None => {
                return JsonRpcResponse::error(
                    id,
                    ERROR_INVALID_PARAMS,
                    "Missing transaction data".to_string(),
                )
            }
        };

        let tx_bytes = match hex::decode(tx_data_str.trim_start_matches("0x")) {
            Ok(b) => b,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    ERROR_INVALID_PARAMS,
                    format!("Failed to decode transaction hex: {}", e),
                )
            }
        };

        let tx: crate::transaction::Transaction = match bincode::deserialize(&tx_bytes) {
            Ok(tx) => tx,
            Err(e) => {
                return JsonRpcResponse::error(
                    id,
                    ERROR_INVALID_PARAMS,
                    format!("Failed to deserialize transaction: {}", e),
                )
            }
        };

        let mut blockchain = self.blockchain.write().await;
        match blockchain.submit_transaction(tx) {
            Ok(tx_hash) => {
                info!("Transaction accepted: 0x{}", hex::encode(tx_hash.as_bytes()));
                let tx_hash_hex = format!("0x{}", hex::encode(tx_hash.as_bytes()));
                JsonRpcResponse::success(id, json!(tx_hash_hex))
            }
            Err(e) => {
                warn!("Transaction rejected: {}", e);
                JsonRpcResponse::error(
                    id,
                    ERROR_INVALID_PARAMS,
                    format!("Transaction validation failed: {}", e),
                )
            }
        }
    }

    pub(crate) async fn handle_get_transaction_count(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };
        let blockchain = self.blockchain.read().await;
        let nonce_hex = format!("0x{:x}", blockchain.get_nonce(&address));
        JsonRpcResponse::success(id, json!(nonce_hex))
    }

    pub(crate) async fn handle_get_block_by_number(&self, id: Value, _params: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let block_info = json!({
            "number": format!("0x{:x}", blockchain.get_height()),
            "hash": format!("0x{}", hex::encode([0u8; 32])),
            "parentHash": format!("0x{}", hex::encode([0u8; 32])),
            "timestamp": format!("0x{:x}", chrono::Utc::now().timestamp()),
            "transactions": [],
            "gasUsed": "0x0",
            "gasLimit": "0x1c9c380",
        });
        JsonRpcResponse::success(id, block_info)
    }
}
