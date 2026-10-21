use serde_json::{json, Value};
use tracing::info;
use crate::rpc::types::*;
use super::helpers::parse_address_param;
use super::RpcHandler;

impl RpcHandler {
    /// token_getSupply() — total/circulating supply info
    pub(crate) async fn handle_token_get_supply(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let supply = blockchain.get_token_supply();
        JsonRpcResponse::success(
            id,
            json!({
                "total_supply": supply.total_supply.to_string(),
                "circulating_supply": supply.circulating_supply.to_string(),
                "total_minted": supply.total_minted.to_string(),
                "total_burned": supply.total_burned.to_string(),
            }),
        )
    }

    /// token_getPrice() — current ISA/USD price from oracle (micro-USD)
    pub(crate) async fn handle_token_get_price(&self, id: Value) -> JsonRpcResponse {
        match &self.oracle {
            Some(oracle_lock) => {
                let current_height = self.blockchain.read().await.get_height();
                let oracle = oracle_lock.read().await;
                match oracle.get_price() {
                    Ok(price) => {
                        let is_stale = oracle.is_stale(current_height);
                        JsonRpcResponse::success(
                            id,
                            json!({
                                "price_micro_usd": price,
                                "price_usd": price as f64 / 1_000_000.0,
                                "stale": is_stale,
                            }),
                        )
                    }
                    Err(e) => JsonRpcResponse::error(
                        id,
                        ERROR_NOT_FOUND,
                        format!("No price available: {}", e),
                    ),
                }
            }
            None => JsonRpcResponse::error(
                id,
                ERROR_NOT_FOUND,
                "Oracle not configured".to_string(),
            ),
        }
    }

    /// staking_getStake(address) — get stake info for address
    pub(crate) async fn handle_staking_get_stake(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match parse_address_param(&params, 0) {
            Ok(a) => a,
            Err(mut e) => {
                e.id = id;
                return e;
            }
        };

        match &self.staking {
            Some(vault_lock) => {
                let vault = vault_lock.read().await;
                match vault.get_stake(&address) {
                    Some(entry) => {
                        let unbonding: Vec<Value> = entry
                            .unbonding
                            .iter()
                            .map(|u| {
                                json!({
                                    "amount": u.amount.to_string(),
                                    "completion_height": u.completion_height,
                                })
                            })
                            .collect();
                        JsonRpcResponse::success(
                            id,
                            json!({
                                "address": format!("0x{}", hex::encode(address.as_bytes())),
                                "staked": entry.amount.to_string(),
                                "staked_at": entry.staked_at,
                                "unbonding": unbonding,
                            }),
                        )
                    }
                    None => JsonRpcResponse::error(
                        id,
                        ERROR_NOT_FOUND,
                        format!("No stake found for address 0x{}", hex::encode(address.as_bytes())),
                    ),
                }
            }
            None => JsonRpcResponse::error(
                id,
                ERROR_NOT_FOUND,
                "Staking vault not configured".to_string(),
            ),
        }
    }

    /// staking_getTotalStaked() — total ISA staked
    pub(crate) async fn handle_staking_get_total_staked(&self, id: Value) -> JsonRpcResponse {
        match &self.staking {
            Some(vault_lock) => {
                let vault = vault_lock.read().await;
                let total = vault.get_total_staked();
                JsonRpcResponse::success(
                    id,
                    json!({
                        "total_staked": total.to_string(),
                    }),
                )
            }
            None => JsonRpcResponse::success(
                id,
                json!({ "total_staked": "0" }),
            ),
        }
    }

    /// staking_getValidators() — list validators with active stakes
    pub(crate) async fn handle_staking_get_validators(&self, id: Value) -> JsonRpcResponse {
        let _blockchain = self.blockchain.read().await;

        JsonRpcResponse::success(
            id,
            json!({
                "validators": [],
                "note": "Validator enumeration requires indexed validator registry — use staking_getStake(address) for individual queries.",
            }),
        )
    }

    /// treasury_getBalance() — treasury balance
    pub(crate) async fn handle_treasury_get_balance(&self, id: Value) -> JsonRpcResponse {
        match &self.treasury {
            Some(treasury_lock) => {
                let treasury = treasury_lock.read().await;
                JsonRpcResponse::success(
                    id,
                    json!({
                        "balance": treasury.get_balance().to_string(),
                    }),
                )
            }
            None => JsonRpcResponse::success(id, json!({ "balance": "0" })),
        }
    }

    /// treasury_getStats() — collected/distributed totals
    pub(crate) async fn handle_treasury_get_stats(&self, id: Value) -> JsonRpcResponse {
        match &self.treasury {
            Some(treasury_lock) => {
                let treasury = treasury_lock.read().await;
                JsonRpcResponse::success(
                    id,
                    json!({
                        "balance": treasury.get_balance().to_string(),
                        "total_collected": treasury.get_total_collected().to_string(),
                        "total_distributed": treasury.get_total_distributed().to_string(),
                        "fee_rate_bps": treasury.get_fee_rate(),
                        "distribution_count": treasury.get_distributions().len(),
                    }),
                )
            }
            None => JsonRpcResponse::success(
                id,
                json!({
                    "balance": "0",
                    "total_collected": "0",
                    "total_distributed": "0",
                    "fee_rate_bps": 0,
                    "distribution_count": 0,
                }),
            ),
        }
    }

    /// system_health() — returns node health snapshot
    pub(crate) async fn handle_system_health(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let height = blockchain.get_height();
        info!("system_health: height={}", height);
        JsonRpcResponse::success(
            id,
            json!({
                "status": "ok",
                "height": height,
                "peers": 0u64,
                "syncing": false,
            }),
        )
    }

    /// system_version() — returns node version and chain ID
    pub(crate) async fn handle_system_version(&self, id: Value) -> JsonRpcResponse {
        info!("system_version: chain_id={}", self.chain_id);
        JsonRpcResponse::success(
            id,
            json!({
                "version": "0.1.0",
                "chain_id": self.chain_id,
            }),
        )
    }

    /// system_metrics() — JSON-RPC handler; returns Prometheus text as a string result.
    pub(crate) async fn handle_system_metrics(&self, id: Value) -> JsonRpcResponse {
        let output = self.render_metrics().await;
        JsonRpcResponse::success(id, json!(output))
    }

    /// Render current Prometheus text exposition metrics.
    ///
    /// Exposed as `pub` so the HTTP `GET /metrics` handler in `server.rs` can
    /// reuse it without duplicating the blockchain read logic.
    pub async fn render_metrics(&self) -> String {
        let blockchain = self.blockchain.read().await;
        let chain_height = blockchain.get_height();
        let mempool_size = blockchain.pending_transaction_count() as u64;
        let account_count = blockchain.account_count() as u64;
        drop(blockchain);

        info!("metrics render: height={}, mempool={}, accounts={}", chain_height, mempool_size, account_count);
        self.metrics.render(chain_height, mempool_size, account_count)
    }
}
