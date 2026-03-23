use serde::{Deserialize, Serialize};
use crate::types::{Hash, Address};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcResponse {
    pub fn success(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: serde_json::Value, code: i32, message: String) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
            id,
        }
    }
}

// ---------------------------------------------------------------------------
// Legacy / eth-compatible method names
// ---------------------------------------------------------------------------
pub const METHOD_CHAIN_ID: &str = "eth_chainId";
pub const METHOD_BLOCK_NUMBER: &str = "eth_blockNumber";
pub const METHOD_GET_BALANCE: &str = "eth_getBalance";
pub const METHOD_SEND_TRANSACTION: &str = "eth_sendTransaction";
pub const METHOD_SEND_RAW_TRANSACTION: &str = "eth_sendRawTransaction";
pub const METHOD_GET_TRANSACTION_COUNT: &str = "eth_getTransactionCount";
pub const METHOD_CALL: &str = "eth_call";
pub const METHOD_GET_BLOCK_BY_NUMBER: &str = "eth_getBlockByNumber";
pub const METHOD_GET_TRANSACTION_RECEIPT: &str = "eth_getTransactionReceipt";

// ---------------------------------------------------------------------------
// Chain methods
// ---------------------------------------------------------------------------
pub const METHOD_CHAIN_GET_BLOCK: &str = "chain_getBlock";
pub const METHOD_CHAIN_GET_BLOCK_BY_HEIGHT: &str = "chain_getBlockByHeight";
pub const METHOD_CHAIN_GET_LATEST_BLOCK: &str = "chain_getLatestBlock";
pub const METHOD_CHAIN_GET_HEIGHT: &str = "chain_getHeight";
pub const METHOD_CHAIN_GET_CHAIN_ID: &str = "chain_getChainId";

// ---------------------------------------------------------------------------
// Wallet / account methods
// ---------------------------------------------------------------------------
pub const METHOD_WALLET_GET_BALANCE: &str = "wallet_getBalance";
pub const METHOD_WALLET_GET_NONCE: &str = "wallet_getNonce";
pub const METHOD_WALLET_GET_ACCOUNT: &str = "wallet_getAccount";

// ---------------------------------------------------------------------------
// Transaction methods
// ---------------------------------------------------------------------------
pub const METHOD_TX_SUBMIT: &str = "tx_submit";
pub const METHOD_TX_GET_TRANSACTION: &str = "tx_getTransaction";
pub const METHOD_TX_GET_PENDING: &str = "tx_getPending";

// ---------------------------------------------------------------------------
// Token methods
// ---------------------------------------------------------------------------
pub const METHOD_TOKEN_GET_SUPPLY: &str = "token_getSupply";
pub const METHOD_TOKEN_GET_PRICE: &str = "token_getPrice";

// ---------------------------------------------------------------------------
// Staking methods
// ---------------------------------------------------------------------------
pub const METHOD_STAKING_GET_STAKE: &str = "staking_getStake";
pub const METHOD_STAKING_GET_TOTAL_STAKED: &str = "staking_getTotalStaked";
pub const METHOD_STAKING_GET_VALIDATORS: &str = "staking_getValidators";

// ---------------------------------------------------------------------------
// Treasury methods
// ---------------------------------------------------------------------------
pub const METHOD_TREASURY_GET_BALANCE: &str = "treasury_getBalance";
pub const METHOD_TREASURY_GET_STATS: &str = "treasury_getStats";

// ---------------------------------------------------------------------------
// System / health methods
// ---------------------------------------------------------------------------
pub const METHOD_SYSTEM_HEALTH: &str = "system_health";
pub const METHOD_SYSTEM_VERSION: &str = "system_version";
pub const METHOD_SYSTEM_METRICS: &str = "system_metrics";

// ---------------------------------------------------------------------------
// JSON-RPC error codes
// ---------------------------------------------------------------------------
pub const ERROR_PARSE: i32 = -32700;
pub const ERROR_INVALID_REQUEST: i32 = -32600;
pub const ERROR_METHOD_NOT_FOUND: i32 = -32601;
pub const ERROR_INVALID_PARAMS: i32 = -32602;
pub const ERROR_INTERNAL: i32 = -32603;
/// Resource not found (application-level, outside standard JSON-RPC range)
pub const ERROR_NOT_FOUND: i32 = -32001;
