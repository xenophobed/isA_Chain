use serde_json::{json, Value};
use crate::types::{Address, Hash};
use crate::rpc::types::*;

#[allow(clippy::result_large_err)]
pub fn parse_address_param(params: &Value, index: usize) -> Result<Address, JsonRpcResponse> {
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
pub fn parse_hash_param(params: &Value, index: usize) -> Result<Hash, JsonRpcResponse> {
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

pub fn block_to_json(block: &crate::block::Block) -> Value {
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

pub fn tx_to_json(tx: &crate::transaction::Transaction, hash: &Hash) -> Value {
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
