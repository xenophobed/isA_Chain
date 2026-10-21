use serde_json::{json, Value};
use crate::rpc::types::*;
use super::helpers::{parse_address_param, parse_hash_param, tx_to_json};
use super::RpcHandler;

impl RpcHandler {
    /// wallet_getBalance(address) — get ISA balance
    pub(crate) async fn handle_wallet_get_balance(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };
        let blockchain = self.blockchain.read().await;
        let balance = blockchain.get_balance(&address);
        JsonRpcResponse::success(
            id,
            json!({
                "address": format!("0x{}", hex::encode(address.as_bytes())),
                "balance": balance.to_string(),
                "balance_hex": format!("0x{:x}", balance),
            }),
        )
    }

    /// wallet_getNonce(address) — get account nonce
    pub(crate) async fn handle_wallet_get_nonce(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };
        let blockchain = self.blockchain.read().await;
        let nonce = blockchain.get_nonce(&address);
        JsonRpcResponse::success(id, json!(nonce))
    }

    /// wallet_getAccount(address) — full account info
    pub(crate) async fn handle_wallet_get_account(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };
        let blockchain = self.blockchain.read().await;
        match blockchain.get_account(&address) {
            Some(account) => {
                let account_type = match &account.account_type {
                    crate::account::AccountType::External => "external",
                    crate::account::AccountType::Contract { .. } => "contract",
                    crate::account::AccountType::System { .. } => "system",
                };
                JsonRpcResponse::success(
                    id,
                    json!({
                        "address": format!("0x{}", hex::encode(address.as_bytes())),
                        "balance": account.balance.to_string(),
                        "nonce": account.nonce,
                        "type": account_type,
                        "is_validator": account.is_validator(),
                    }),
                )
            }
            None => JsonRpcResponse::error(
                id,
                ERROR_NOT_FOUND,
                format!("Account not found: 0x{}", hex::encode(address.as_bytes())),
            ),
        }
    }

    /// tx_submit(tx_data) — submit a raw transaction (same as eth_sendRawTransaction)
    pub(crate) async fn handle_tx_submit(&self, id: Value, params: Value) -> JsonRpcResponse {
        self.handle_send_raw_transaction(id, params).await
    }

    /// tx_getTransaction(hash) — get a pending transaction by hash
    pub(crate) async fn handle_tx_get_transaction(&self, id: Value, params: Value) -> JsonRpcResponse {
        let hash = match parse_hash_param(&params, 0) {
            Ok(h) => h,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };

        let blockchain = self.blockchain.read().await;
        match blockchain.get_pending_transaction(&hash) {
            Some(tx) => JsonRpcResponse::success(id, tx_to_json(tx, &hash)),
            None => JsonRpcResponse::error(
                id,
                ERROR_NOT_FOUND,
                format!("Transaction not found: 0x{}", hex::encode(hash.as_bytes())),
            ),
        }
    }

    /// tx_getPending() — list all pending transactions
    pub(crate) async fn handle_tx_get_pending(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let txs = blockchain.get_pending_transactions(1000);
        let tx_list: Vec<Value> = txs
            .iter()
            .map(|tx| {
                let hash = tx.hash();
                tx_to_json(tx, &hash)
            })
            .collect();
        JsonRpcResponse::success(id, json!(tx_list))
    }
}
