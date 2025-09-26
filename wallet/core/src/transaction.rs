use isa_chain_core::transaction::Transaction;
use isa_chain_core::types::{Address, Amount, Hash};
use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionRecord {
    pub hash: Hash,
    pub from: Address,
    pub to: Option<Address>,
    pub amount: Amount,
    pub gas_used: u64,
    pub gas_price: u64,
    pub status: TransactionStatus,
    pub timestamp: DateTime<Utc>,
    pub block_height: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum TransactionStatus {
    Pending,
    Confirmed,
    Failed,
    Dropped,
}

// TODO: Implement transaction history management
// TODO: Implement transaction broadcasting
// TODO: Implement transaction monitoring