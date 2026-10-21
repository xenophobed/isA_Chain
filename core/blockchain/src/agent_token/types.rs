use std::collections::HashMap;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use super::curve::BondingCurve;

/// Errors for agent token operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentTokenError {
    #[error("Token already exists for agent: {0:?}")]
    TokenAlreadyExists(Hash),

    #[error("Token not found for agent: {0:?}")]
    TokenNotFound(Hash),

    #[error("Insufficient funds")]
    InsufficientFunds,

    #[error("Insufficient tokens")]
    InsufficientTokens,

    #[error("Invalid amount")]
    InvalidAmount,

    #[error("Unauthorized: {0}")]
    Unauthorized(Address),

    #[error("Zero amount not allowed")]
    ZeroAmount,
}

/// On-chain bonding-curve token for a single agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentToken {
    /// The agent this token represents
    pub agent_id: Hash,
    /// Circulating supply of agent tokens
    pub total_supply: Amount,
    /// ISA locked as reserve (tracks curve integral)
    pub reserve_balance: Amount,
    /// token holder → balance
    pub holders: HashMap<Address, Amount>,
    /// Pricing curve
    pub curve_type: BondingCurve,
    /// Block when the token was created
    pub created_at: BlockHeight,
    /// Address that created this token
    pub creator: Address,
}

impl AgentToken {
    pub(crate) fn new(agent_id: Hash, curve: BondingCurve, creator: Address, height: BlockHeight) -> Self {
        Self {
            agent_id,
            total_supply: 0,
            reserve_balance: 0,
            holders: HashMap::new(),
            curve_type: curve,
            created_at: height,
            creator,
        }
    }
}
