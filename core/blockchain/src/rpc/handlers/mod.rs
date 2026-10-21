use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use crate::blockchain::Blockchain;
use crate::metrics::ChainMetrics;
use crate::oracle::PriceOracle;
use crate::staking::StakingVault;
use crate::treasury::ProtocolTreasury;
use crate::types::ChainId;
use super::types::*;

pub mod chain;
pub mod ecosystem;
pub mod helpers;
pub mod legacy;
pub mod wallet_tx;

#[cfg(test)]
mod tests;

pub struct RpcHandler {
    pub(crate) blockchain: Arc<RwLock<Blockchain>>,
    pub(crate) chain_id: ChainId,
    pub(crate) staking: Option<Arc<RwLock<StakingVault>>>,
    pub(crate) treasury: Option<Arc<RwLock<ProtocolTreasury>>>,
    pub(crate) oracle: Option<Arc<RwLock<PriceOracle>>>,
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
}
