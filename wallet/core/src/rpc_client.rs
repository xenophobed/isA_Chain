//! RPC client for communicating with isA_Chain nodes.
//!
//! This module provides a request-builder / response-parser approach so the
//! caller controls transport. `RpcClient` constructs typed `RpcRequest` values
//! and parses raw JSON strings into `serde_json::Value` (or an `RpcClientError`
//! when the server returned a JSON-RPC error object).

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur in the RPC client layer.
#[derive(Debug, Error)]
pub enum RpcClientError {
    /// Could not reach the node.
    #[error("connection failed: {0}")]
    ConnectionFailed(String),

    /// The request timed out.
    #[error("request timed out")]
    Timeout,

    /// The response could not be understood.
    #[error("invalid response: {0}")]
    InvalidResponse(String),

    /// The node returned a JSON-RPC error object.
    #[error("rpc error {code}: {message}")]
    RpcError { code: i32, message: String },

    /// Could not serialize the request or deserialize the response.
    #[error("serialization error: {0}")]
    SerializationError(String),

    /// All retry attempts were exhausted.
    #[error("max retries exceeded")]
    MaxRetriesExceeded,
}

// ---------------------------------------------------------------------------
// Wire types
// ---------------------------------------------------------------------------

/// A JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: serde_json::Value,
    pub id: u64,
}

/// A JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<RpcErrorResponse>,
    pub id: u64,
}

/// The error object embedded in an `RpcResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcErrorResponse {
    pub code: i32,
    pub message: String,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for creating an `RpcClient`.
#[derive(Debug, Clone)]
pub struct RpcClientConfig {
    /// HTTP endpoint URL of the isA_Chain node (e.g. `"http://localhost:8545"`).
    pub url: String,
    /// Number of times to retry a failed request (default: 3).
    pub max_retries: u32,
    /// Request timeout in milliseconds (default: 5000).
    pub timeout_ms: u64,
    /// Maximum number of concurrent connections (hint for pool, default: 10).
    pub max_connections: u32,
}

impl Default for RpcClientConfig {
    fn default() -> Self {
        Self {
            url: "http://localhost:8545".to_string(),
            max_retries: 3,
            timeout_ms: 5000,
            max_connections: 10,
        }
    }
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Synchronous RPC client that builds JSON-RPC requests and parses responses.
///
/// Because the wallet crate does not depend on an async HTTP client, this type
/// is responsible for constructing `RpcRequest` values (which the caller sends
/// over any transport) and for parsing the raw JSON strings returned by the
/// node into Rust values.
pub struct RpcClient {
    /// RPC endpoint URL.
    pub url: String,
    /// Maximum retry count.
    pub max_retries: u32,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
    /// Monotonically increasing request ID counter.
    request_id: AtomicU64,
}

impl RpcClient {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new `RpcClient` from the provided configuration.
    pub fn new(config: RpcClientConfig) -> Self {
        Self {
            url: config.url,
            max_retries: config.max_retries,
            timeout_ms: config.timeout_ms,
            request_id: AtomicU64::new(1),
        }
    }

    // -----------------------------------------------------------------------
    // Core helpers
    // -----------------------------------------------------------------------

    /// Build a raw `RpcRequest` for the given method and params.
    ///
    /// Each call increments the internal request-ID counter.
    pub fn build_request(&self, method: &str, params: serde_json::Value) -> RpcRequest {
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        RpcRequest {
            jsonrpc: "2.0".to_string(),
            method: method.to_string(),
            params,
            id,
        }
    }

    /// Parse a raw JSON-RPC response string, returning the `result` value on
    /// success or an `RpcClientError` on a JSON-RPC error or parse failure.
    pub fn parse_response(&self, response_json: &str) -> Result<serde_json::Value, RpcClientError> {
        let response: RpcResponse = serde_json::from_str(response_json).map_err(|e| {
            RpcClientError::InvalidResponse(format!("failed to parse JSON: {}", e))
        })?;

        if let Some(err) = response.error {
            return Err(RpcClientError::RpcError {
                code: err.code,
                message: err.message,
            });
        }

        response
            .result
            .ok_or_else(|| RpcClientError::InvalidResponse("missing 'result' field".to_string()))
    }

    // -----------------------------------------------------------------------
    // Convenience request builders
    // -----------------------------------------------------------------------

    /// Build a `wallet_getBalance` request for `address`.
    pub fn get_balance_request(&self, address: &str) -> RpcRequest {
        self.build_request(
            "wallet_getBalance",
            serde_json::json!([address]),
        )
    }

    /// Build a `wallet_getNonce` request for `address`.
    pub fn get_nonce_request(&self, address: &str) -> RpcRequest {
        self.build_request(
            "wallet_getNonce",
            serde_json::json!([address]),
        )
    }

    /// Build a `wallet_getAccount` request for `address`.
    pub fn get_account_request(&self, address: &str) -> RpcRequest {
        self.build_request(
            "wallet_getAccount",
            serde_json::json!([address]),
        )
    }

    /// Build a `tx_submit` request carrying raw hex-encoded transaction bytes.
    pub fn submit_tx_request(&self, tx_hex: &str) -> RpcRequest {
        self.build_request(
            "tx_submit",
            serde_json::json!([tx_hex]),
        )
    }

    /// Build a `tx_getTransaction` request for `hash`.
    pub fn get_tx_request(&self, hash: &str) -> RpcRequest {
        self.build_request(
            "tx_getTransaction",
            serde_json::json!([hash]),
        )
    }

    /// Build a `chain_getBlock` request for `hash`.
    pub fn get_block_request(&self, hash: &str) -> RpcRequest {
        self.build_request(
            "chain_getBlock",
            serde_json::json!([hash]),
        )
    }

    /// Build a `chain_getBlockByHeight` request for `height`.
    pub fn get_block_by_height_request(&self, height: u64) -> RpcRequest {
        self.build_request(
            "chain_getBlockByHeight",
            serde_json::json!([height]),
        )
    }

    /// Build a `chain_getLatestBlock` request (no params).
    pub fn get_latest_block_request(&self) -> RpcRequest {
        self.build_request("chain_getLatestBlock", serde_json::json!([]))
    }

    /// Build a `chain_getHeight` request (no params).
    pub fn get_height_request(&self) -> RpcRequest {
        self.build_request("chain_getHeight", serde_json::json!([]))
    }

    /// Build a `token_getSupply` request (no params).
    pub fn get_token_supply_request(&self) -> RpcRequest {
        self.build_request("token_getSupply", serde_json::json!([]))
    }

    /// Build a `token_getPrice` request (no params).
    pub fn get_token_price_request(&self) -> RpcRequest {
        self.build_request("token_getPrice", serde_json::json!([]))
    }

    /// Build a `staking_getStake` request for `address`.
    pub fn get_stake_request(&self, address: &str) -> RpcRequest {
        self.build_request(
            "staking_getStake",
            serde_json::json!([address]),
        )
    }

    /// Build a `tx_getPending` request (no params).
    pub fn get_pending_txs_request(&self) -> RpcRequest {
        self.build_request("tx_getPending", serde_json::json!([]))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_client() -> RpcClient {
        RpcClient::new(RpcClientConfig {
            url: "http://localhost:8545".to_string(),
            max_retries: 3,
            timeout_ms: 5000,
            max_connections: 10,
        })
    }

    // -----------------------------------------------------------------------
    // Core helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_build_request() {
        let client = make_client();
        let req = client.build_request("eth_blockNumber", serde_json::json!([]));
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "eth_blockNumber");
        assert_eq!(req.params, serde_json::json!([]));
        assert!(req.id > 0);
    }

    #[test]
    fn test_parse_success_response() {
        let client = make_client();
        let json = r#"{"jsonrpc":"2.0","result":"0x1234","id":1}"#;
        let result = client.parse_response(json).unwrap();
        assert_eq!(result, serde_json::json!("0x1234"));
    }

    #[test]
    fn test_parse_error_response() {
        let client = make_client();
        let json = r#"{"jsonrpc":"2.0","error":{"code":-32601,"message":"Method not found"},"id":1}"#;
        let err = client.parse_response(json).unwrap_err();
        match err {
            RpcClientError::RpcError { code, message } => {
                assert_eq!(code, -32601);
                assert_eq!(message, "Method not found");
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_invalid_json() {
        let client = make_client();
        let err = client.parse_response("not json at all").unwrap_err();
        assert!(matches!(err, RpcClientError::InvalidResponse(_)));
    }

    #[test]
    fn test_request_id_increments() {
        let client = make_client();
        let req1 = client.build_request("method_a", serde_json::json!([]));
        let req2 = client.build_request("method_b", serde_json::json!([]));
        let req3 = client.build_request("method_c", serde_json::json!([]));
        assert!(req2.id > req1.id);
        assert!(req3.id > req2.id);
        // IDs must be strictly sequential
        assert_eq!(req2.id, req1.id + 1);
        assert_eq!(req3.id, req2.id + 1);
    }

    // -----------------------------------------------------------------------
    // Convenience builders
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_balance_request() {
        let client = make_client();
        let addr = "0xdeadbeef";
        let req = client.get_balance_request(addr);
        assert_eq!(req.method, "wallet_getBalance");
        assert_eq!(req.params, serde_json::json!([addr]));
    }

    #[test]
    fn test_get_nonce_request() {
        let client = make_client();
        let addr = "0xcafebabe";
        let req = client.get_nonce_request(addr);
        assert_eq!(req.method, "wallet_getNonce");
        assert_eq!(req.params, serde_json::json!([addr]));
    }

    #[test]
    fn test_submit_tx_request() {
        let client = make_client();
        let tx_hex = "deadbeef01020304";
        let req = client.submit_tx_request(tx_hex);
        assert_eq!(req.method, "tx_submit");
        assert_eq!(req.params, serde_json::json!([tx_hex]));
    }

    #[test]
    fn test_get_block_request() {
        let client = make_client();
        let hash = "0xaabbccdd";
        let req = client.get_block_request(hash);
        assert_eq!(req.method, "chain_getBlock");
        assert_eq!(req.params, serde_json::json!([hash]));
    }

    #[test]
    fn test_all_convenience_methods() {
        let client = make_client();

        // Verify method names are correct for every convenience builder.
        assert_eq!(client.get_balance_request("a").method, "wallet_getBalance");
        assert_eq!(client.get_nonce_request("a").method, "wallet_getNonce");
        assert_eq!(client.get_account_request("a").method, "wallet_getAccount");
        assert_eq!(client.submit_tx_request("ff").method, "tx_submit");
        assert_eq!(client.get_tx_request("h").method, "tx_getTransaction");
        assert_eq!(client.get_block_request("h").method, "chain_getBlock");
        assert_eq!(client.get_block_by_height_request(42).method, "chain_getBlockByHeight");
        assert_eq!(client.get_latest_block_request().method, "chain_getLatestBlock");
        assert_eq!(client.get_height_request().method, "chain_getHeight");
        assert_eq!(client.get_token_supply_request().method, "token_getSupply");
        assert_eq!(client.get_token_price_request().method, "token_getPrice");
        assert_eq!(client.get_stake_request("a").method, "staking_getStake");
        assert_eq!(client.get_pending_txs_request().method, "tx_getPending");
    }
}
