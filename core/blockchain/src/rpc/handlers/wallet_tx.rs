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

    /// faucet_request(address, [amount]) — transfer ISA tokens from testnet faucet to address
    pub(crate) async fn handle_faucet_request(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };

        // Default: 100 ISA (100 * 10^18 wei)
        let default_amount: crate::types::Amount = 100_000_000_000_000_000_000;
        let amount: crate::types::Amount = if let Some(arr) = params.as_array() {
            if let Some(val) = arr.get(1) {
                if let Some(num) = val.as_u64() {
                    num as u128
                } else if let Some(s) = val.as_str() {
                    s.parse::<u128>().unwrap_or(default_amount)
                } else {
                    default_amount
                }
            } else {
                default_amount
            }
        } else {
            default_amount
        };

        let mut blockchain = self.blockchain.write().await;
        let height = blockchain.get_height();
        match blockchain.faucet_transfer(address, amount) {
            Ok(new_balance) => {
                tracing::info!("Faucet drip of {} ISA to 0x{}", amount, hex::encode(address.as_bytes()));
                JsonRpcResponse::success(
                    id,
                    json!({
                        "recipient": format!("0x{}", hex::encode(address.as_bytes())),
                        "amount_drip": amount.to_string(),
                        "new_balance": new_balance.to_string(),
                        "status": "success",
                        "tx_hash": format!("0x{}", hex::encode(crate::types::Hash::hash_data(format!("faucet:{}:{}", hex::encode(address.as_bytes()), height).as_bytes()).as_bytes()))
                    }),
                )
            }
            Err(e) => {
                JsonRpcResponse::error(id, ERROR_INTERNAL, format!("Faucet transfer failed: {}", e))
            }
        }
    }
}
