use serde_json::{json, Value};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::blockchain::Blockchain;
use crate::metrics::ChainMetrics;
use crate::oracle::PriceOracle;
use crate::staking::StakingVault;
use crate::treasury::ProtocolTreasury;
use crate::types::{Address, ChainId, Hash};
use super::types::*;

// ---------------------------------------------------------------------------
// RpcHandler — central dispatch
// ---------------------------------------------------------------------------

pub struct RpcHandler {
    blockchain: Arc<RwLock<Blockchain>>,
    chain_id: ChainId,
    /// Optional staking vault (may be None when running in test / minimal mode)
    staking: Option<Arc<RwLock<StakingVault>>>,
    /// Optional treasury
    treasury: Option<Arc<RwLock<ProtocolTreasury>>>,
    /// Optional price oracle
    oracle: Option<Arc<RwLock<PriceOracle>>>,
    /// Prometheus-compatible metrics counters
    pub metrics: ChainMetrics,
}

impl RpcHandler {
    pub fn new(blockchain: Arc<RwLock<Blockchain>>, chain_id: ChainId) -> Self {
        Self {
            blockchain,
            chain_id,
            staking: None,
            treasury: None,
            oracle: None,
            metrics: ChainMetrics::new(),
        }
    }

    /// Attach a staking vault so staking RPC methods return real data.
    pub fn with_staking(mut self, vault: Arc<RwLock<StakingVault>>) -> Self {
        self.staking = Some(vault);
        self
    }

    /// Attach a treasury so treasury RPC methods return real data.
    pub fn with_treasury(mut self, treasury: Arc<RwLock<ProtocolTreasury>>) -> Self {
        self.treasury = Some(treasury);
        self
    }

    /// Attach a price oracle so token price queries return real data.
    pub fn with_oracle(mut self, oracle: Arc<RwLock<PriceOracle>>) -> Self {
        self.oracle = Some(oracle);
        self
    }

    // -----------------------------------------------------------------------
    // Dispatch
    // -----------------------------------------------------------------------

    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        debug!("RPC request received: method={}, id={:?}", req.method, req.id);
        info!("RPC request: method={}, id={:?}", req.method, req.id);

        // Track every inbound request regardless of outcome.
        self.metrics.inc_rpc_requests();

        match req.method.as_str() {
            // ---- Legacy / eth-compatible methods ----
            METHOD_CHAIN_ID => self.handle_chain_id(req.id).await,
            METHOD_BLOCK_NUMBER => self.handle_block_number(req.id).await,
            METHOD_GET_BALANCE => self.handle_get_balance(req.id, req.params).await,
            METHOD_SEND_RAW_TRANSACTION => self.handle_send_raw_transaction(req.id, req.params).await,
            METHOD_GET_TRANSACTION_COUNT => self.handle_get_transaction_count(req.id, req.params).await,
            METHOD_GET_BLOCK_BY_NUMBER => self.handle_get_block_by_number(req.id, req.params).await,

            // ---- Chain methods ----
            METHOD_CHAIN_GET_BLOCK => self.handle_chain_get_block(req.id, req.params).await,
            METHOD_CHAIN_GET_BLOCK_BY_HEIGHT => self.handle_chain_get_block_by_height(req.id, req.params).await,
            METHOD_CHAIN_GET_LATEST_BLOCK => self.handle_chain_get_latest_block(req.id).await,
            METHOD_CHAIN_GET_HEIGHT => self.handle_chain_get_height(req.id).await,
            METHOD_CHAIN_GET_CHAIN_ID => self.handle_chain_get_chain_id(req.id).await,

            // ---- Wallet / account methods ----
            METHOD_WALLET_GET_BALANCE => self.handle_wallet_get_balance(req.id, req.params).await,
            METHOD_WALLET_GET_NONCE => self.handle_wallet_get_nonce(req.id, req.params).await,
            METHOD_WALLET_GET_ACCOUNT => self.handle_wallet_get_account(req.id, req.params).await,

            // ---- Transaction methods ----
            METHOD_TX_SUBMIT => self.handle_tx_submit(req.id, req.params).await,
            METHOD_TX_GET_TRANSACTION => self.handle_tx_get_transaction(req.id, req.params).await,
            METHOD_TX_GET_PENDING => self.handle_tx_get_pending(req.id).await,

            // ---- Token methods ----
            METHOD_TOKEN_GET_SUPPLY => self.handle_token_get_supply(req.id).await,
            METHOD_TOKEN_GET_PRICE => self.handle_token_get_price(req.id).await,

            // ---- Staking methods ----
            METHOD_STAKING_GET_STAKE => self.handle_staking_get_stake(req.id, req.params).await,
            METHOD_STAKING_GET_TOTAL_STAKED => self.handle_staking_get_total_staked(req.id).await,
            METHOD_STAKING_GET_VALIDATORS => self.handle_staking_get_validators(req.id).await,

            // ---- Treasury methods ----
            METHOD_TREASURY_GET_BALANCE => self.handle_treasury_get_balance(req.id).await,
            METHOD_TREASURY_GET_STATS => self.handle_treasury_get_stats(req.id).await,

            // ---- System / health methods ----
            METHOD_SYSTEM_HEALTH => self.handle_system_health(req.id).await,
            METHOD_SYSTEM_VERSION => self.handle_system_version(req.id).await,
            METHOD_SYSTEM_METRICS => self.handle_system_metrics(req.id).await,

            _ => {
                warn!("Unknown RPC method: {}", req.method);
                JsonRpcResponse::error(
                    req.id,
                    ERROR_METHOD_NOT_FOUND,
                    format!("Method {} not found", req.method),
                )
            }
        }
    }

    // -----------------------------------------------------------------------
    // Helper: parse hex address from a params array slot
    // -----------------------------------------------------------------------

    #[allow(clippy::result_large_err)]
    fn parse_address_param(params: &Value, index: usize) -> Result<Address, JsonRpcResponse> {
        let arr = params.as_array().ok_or_else(|| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                "params must be an array".to_string(),
            )
        })?;

        let raw = arr.get(index).and_then(|v| v.as_str()).ok_or_else(|| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                format!("Missing or invalid address at params[{}]", index),
            )
        })?;

        let raw = raw.trim_start_matches("0x");
        let bytes = hex::decode(raw).map_err(|_| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                "Invalid address hex encoding".to_string(),
            )
        })?;

        Address::from_bytes(&bytes).map_err(|e| {
            JsonRpcResponse::error(Value::Null, ERROR_INVALID_PARAMS, e.to_string())
        })
    }

    #[allow(clippy::result_large_err)]
    fn parse_hash_param(params: &Value, index: usize) -> Result<Hash, JsonRpcResponse> {
        let arr = params.as_array().ok_or_else(|| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                "params must be an array".to_string(),
            )
        })?;

        let raw = arr.get(index).and_then(|v| v.as_str()).ok_or_else(|| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                format!("Missing or invalid hash at params[{}]", index),
            )
        })?;

        let raw = raw.trim_start_matches("0x");
        let bytes = hex::decode(raw).map_err(|_| {
            JsonRpcResponse::error(
                Value::Null,
                ERROR_INVALID_PARAMS,
                "Invalid hash hex encoding".to_string(),
            )
        })?;

        Hash::from_bytes(&bytes).map_err(|e| {
            JsonRpcResponse::error(Value::Null, ERROR_INVALID_PARAMS, e.to_string())
        })
    }

    // -----------------------------------------------------------------------
    // Legacy / eth-compatible handlers
    // -----------------------------------------------------------------------

    async fn handle_chain_id(&self, id: Value) -> JsonRpcResponse {
        let chain_id_hex = format!("0x{:x}", self.chain_id);
        JsonRpcResponse::success(id, json!(chain_id_hex))
    }

    async fn handle_block_number(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        let block_number_hex = format!("0x{:x}", blockchain.get_height());
        JsonRpcResponse::success(id, json!(block_number_hex))
    }

    async fn handle_get_balance(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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

    async fn handle_send_raw_transaction(&self, id: Value, params: Value) -> JsonRpcResponse {
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

    async fn handle_get_transaction_count(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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

    async fn handle_get_block_by_number(&self, id: Value, _params: Value) -> JsonRpcResponse {
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

    // -----------------------------------------------------------------------
    // Chain methods
    // -----------------------------------------------------------------------

    /// chain_getBlock(hash) — get block by hash
    async fn handle_chain_get_block(&self, id: Value, params: Value) -> JsonRpcResponse {
        let hash = match Self::parse_hash_param(&params, 0) {
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
    async fn handle_chain_get_block_by_height(&self, id: Value, params: Value) -> JsonRpcResponse {
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
    async fn handle_chain_get_latest_block(&self, id: Value) -> JsonRpcResponse {
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
    async fn handle_chain_get_height(&self, id: Value) -> JsonRpcResponse {
        let blockchain = self.blockchain.read().await;
        JsonRpcResponse::success(id, json!(blockchain.get_height()))
    }

    /// chain_getChainId() — chain ID
    async fn handle_chain_get_chain_id(&self, id: Value) -> JsonRpcResponse {
        JsonRpcResponse::success(id, json!(self.chain_id))
    }

    // -----------------------------------------------------------------------
    // Wallet / account methods
    // -----------------------------------------------------------------------

    /// wallet_getBalance(address) — get ISA balance
    async fn handle_wallet_get_balance(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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
    async fn handle_wallet_get_nonce(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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
    async fn handle_wallet_get_account(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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

    // -----------------------------------------------------------------------
    // Transaction methods
    // -----------------------------------------------------------------------

    /// tx_submit(tx_data) — submit a raw transaction (same as eth_sendRawTransaction)
    async fn handle_tx_submit(&self, id: Value, params: Value) -> JsonRpcResponse {
        self.handle_send_raw_transaction(id, params).await
    }

    /// tx_getTransaction(hash) — get a pending transaction by hash
    async fn handle_tx_get_transaction(&self, id: Value, params: Value) -> JsonRpcResponse {
        let hash = match Self::parse_hash_param(&params, 0) {
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
    async fn handle_tx_get_pending(&self, id: Value) -> JsonRpcResponse {
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

    // -----------------------------------------------------------------------
    // Token methods
    // -----------------------------------------------------------------------

    /// token_getSupply() — total/circulating supply info
    async fn handle_token_get_supply(&self, id: Value) -> JsonRpcResponse {
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
    async fn handle_token_get_price(&self, id: Value) -> JsonRpcResponse {
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

    // -----------------------------------------------------------------------
    // Staking methods
    // -----------------------------------------------------------------------

    /// staking_getStake(address) — get stake info for address
    async fn handle_staking_get_stake(&self, id: Value, params: Value) -> JsonRpcResponse {
        let address = match Self::parse_address_param(&params, 0) {
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
    async fn handle_staking_get_total_staked(&self, id: Value) -> JsonRpcResponse {
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
    async fn handle_staking_get_validators(&self, id: Value) -> JsonRpcResponse {
        // The Blockchain's account map tracks validator status; StakingVault
        // doesn't store addresses directly.  We read the blockchain accounts.
        let _blockchain = self.blockchain.read().await;

        // The Blockchain struct does not expose a direct iterator over accounts.
        // We return an empty list for now with a note; full validator indexing
        // would require exposing blockchain.accounts externally.
        // This satisfies the contract without panicking.
        JsonRpcResponse::success(
            id,
            json!({
                "validators": [],
                "note": "Validator enumeration requires indexed validator registry — use staking_getStake(address) for individual queries.",
            }),
        )
    }

    // -----------------------------------------------------------------------
    // Treasury methods
    // -----------------------------------------------------------------------

    /// treasury_getBalance() — treasury balance
    async fn handle_treasury_get_balance(&self, id: Value) -> JsonRpcResponse {
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

    // -----------------------------------------------------------------------
    // System / health methods
    // -----------------------------------------------------------------------

    /// system_health() — returns node health snapshot
    async fn handle_system_health(&self, id: Value) -> JsonRpcResponse {
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
    async fn handle_system_version(&self, id: Value) -> JsonRpcResponse {
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
    async fn handle_system_metrics(&self, id: Value) -> JsonRpcResponse {
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

    /// treasury_getStats() — collected/distributed totals
    async fn handle_treasury_get_stats(&self, id: Value) -> JsonRpcResponse {
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
}

// ---------------------------------------------------------------------------
// Serialisation helpers
// ---------------------------------------------------------------------------

fn block_to_json(block: &crate::block::Block) -> Value {
    let tx_hashes: Vec<String> = block
        .transactions
        .iter()
        .map(|tx| format!("0x{}", hex::encode(tx.hash().as_bytes())))
        .collect();

    json!({
        "hash": format!("0x{}", hex::encode(block.hash().as_bytes())),
        "height": block.header.height,
        "parent_hash": format!("0x{}", hex::encode(block.header.parent_hash.as_bytes())),
        "timestamp": block.header.timestamp,
        "proposer": format!("0x{}", hex::encode(block.header.proposer.as_bytes())),
        "gas_limit": block.header.gas_limit,
        "gas_used": block.header.gas_used,
        "transactions": tx_hashes,
        "transaction_count": block.transactions.len(),
    })
}

fn tx_to_json(tx: &crate::transaction::Transaction, hash: &Hash) -> Value {
    let tx_type = match &tx.data {
        crate::transaction::TransactionData::Transfer { to, amount, .. } => {
            json!({
                "type": "Transfer",
                "to": format!("0x{}", hex::encode(to.as_bytes())),
                "amount": amount.to_string(),
            })
        }
        _ => json!({ "type": "Other" }),
    };

    json!({
        "hash": format!("0x{}", hex::encode(hash.as_bytes())),
        "from": format!("0x{}", hex::encode(tx.from.as_bytes())),
        "nonce": tx.nonce,
        "gas_limit": tx.gas_limit,
        "gas_price": tx.gas_price,
        "data": tx_type,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::{MAIN_CHAIN_ID, VALIDATOR_MIN_STAKE};
    use serde_json::json;

    fn make_handler() -> RpcHandler {
        let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
        RpcHandler::new(blockchain, MAIN_CHAIN_ID)
    }

    fn make_handler_with_staking() -> RpcHandler {
        let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
        let vault = Arc::new(RwLock::new(StakingVault::default_vault()));
        RpcHandler::new(blockchain, MAIN_CHAIN_ID).with_staking(vault)
    }

    fn make_handler_with_treasury() -> RpcHandler {
        let blockchain = Arc::new(RwLock::new(Blockchain::new(MAIN_CHAIN_ID)));
        let treasury = Arc::new(RwLock::new(ProtocolTreasury::new(
            250,
            Address::from([0xAA; 20]),
        )));
        RpcHandler::new(blockchain, MAIN_CHAIN_ID).with_treasury(treasury)
    }

    fn req(method: &str, params: Value) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id: json!(1),
        }
    }

    // ---- chain_getChainId ----

    #[tokio::test]
    async fn test_chain_get_chain_id() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_CHAIN_GET_CHAIN_ID, json!([])))
            .await;
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap(), json!(MAIN_CHAIN_ID));
    }

    // ---- chain_getHeight ----

    #[tokio::test]
    async fn test_chain_get_height() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_CHAIN_GET_HEIGHT, json!([])))
            .await;
        assert!(resp.error.is_none());
        assert_eq!(resp.result.unwrap(), json!(0u64));
    }

    // ---- chain_getLatestBlock ----

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

    // ---- chain_getBlockByHeight ----

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

    // ---- chain_getBlock ----

    #[tokio::test]
    async fn test_chain_get_block_not_found() {
        let handler = make_handler();
        let fake_hash = hex::encode([0u8; 32]);
        let resp = handler
            .handle_request(req(METHOD_CHAIN_GET_BLOCK, json!([format!("0x{}", fake_hash)])))
            .await;
        // Genesis has a non-zero hash — zero hash is NOT found
        assert!(resp.error.is_some());
    }

    // ---- wallet_getBalance ----

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

    // ---- wallet_getNonce ----

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

    // ---- wallet_getAccount ----

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

    // ---- tx_getPending ----

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

    // ---- token_getSupply ----

    #[tokio::test]
    async fn test_token_get_supply() {
        use crate::types::constants::INITIAL_SUPPLY;
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_TOKEN_GET_SUPPLY, json!([])))
            .await;
        assert!(resp.error.is_none());
        let result = resp.result.unwrap();
        assert_eq!(result["total_supply"], json!(INITIAL_SUPPLY.to_string()));
        assert_eq!(result["total_burned"], json!("0"));
    }

    // ---- token_getPrice (no oracle) ----

    #[tokio::test]
    async fn test_token_get_price_no_oracle() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_TOKEN_GET_PRICE, json!([])))
            .await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ERROR_NOT_FOUND);
    }

    // ---- staking_getTotalStaked (no vault) ----

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

    // ---- staking_getStake ----

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

    // ---- treasury_getBalance ----

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

    // ---- invalid params ----

    #[tokio::test]
    async fn test_invalid_address_returns_error() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_WALLET_GET_BALANCE, json!(["not-a-valid-address"])))
            .await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ERROR_INVALID_PARAMS);
    }

    // ---- unknown method ----

    #[tokio::test]
    async fn test_unknown_method_returns_not_found() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req("made_up_method", json!([])))
            .await;
        assert!(resp.error.is_some());
        assert_eq!(resp.error.unwrap().code, ERROR_METHOD_NOT_FOUND);
    }

    // ---- system_health ----

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

    // ---- system_version ----

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

    // ---- system_metrics ----

    #[tokio::test]
    async fn test_system_metrics_rpc_returns_prometheus_text() {
        let handler = make_handler();
        let resp = handler
            .handle_request(req(METHOD_SYSTEM_METRICS, json!([])))
            .await;
        assert!(resp.error.is_none());

        // Result is a JSON string containing Prometheus exposition text
        let text = resp.result.unwrap();
        let text = text.as_str().expect("system_metrics result should be a string");

        assert!(text.contains("isa_chain_height"), "missing isa_chain_height");
        assert!(text.contains("isa_chain_mempool_size"), "missing isa_chain_mempool_size");
        assert!(text.contains("isa_chain_rpc_requests_total"), "missing rpc counter");
        // After the call above, rpc_requests_total should be at least 1
        assert!(text.contains("isa_chain_rpc_requests_total 1"), "counter should be 1 after first request");
    }

    #[tokio::test]
    async fn test_system_metrics_rpc_counter_increments() {
        let handler = make_handler();

        // Make several requests
        for _ in 0..3 {
            handler
                .handle_request(req(METHOD_SYSTEM_HEALTH, json!([])))
                .await;
        }

        // Now call system_metrics — this is request #4
        let resp = handler
            .handle_request(req(METHOD_SYSTEM_METRICS, json!([])))
            .await;
        assert!(resp.error.is_none());

        let text = resp.result.unwrap();
        let text = text.as_str().unwrap();
        // 3 health requests + 1 metrics request = 4
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

        // Every non-comment, non-empty line must be "name value"
        for line in text.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split_whitespace().collect();
            assert_eq!(parts.len(), 2, "bad prometheus line: {line}");
            parts[1].parse::<u64>().expect("metric value must be u64");
        }
    }
}
