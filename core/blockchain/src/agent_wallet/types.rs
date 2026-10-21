use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};

/// ~24 hours at 3-second block times
pub const DAILY_BLOCKS: u64 = 28_800;
/// ~30 days at 3-second block times
pub const MONTHLY_BLOCKS: u64 = 864_000;

/// Status of an agent wallet
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletStatus {
    /// Wallet is active and can spend
    Active,
    /// Wallet has been frozen by the owner
    Frozen,
    /// Wallet has been suspended by the protocol
    Suspended,
    /// Wallet has been closed; no further operations allowed
    Closed,
}

/// A smart wallet scoped to a single agent instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentWallet {
    /// Wallet address (deterministically derived from agent_id)
    pub address: Address,
    /// Unique identifier for the agent that owns this wallet
    pub agent_id: Hash,
    /// Human owner who controls this wallet
    pub owner: Address,
    /// ISA token balance
    pub balance: Amount,
    /// Credit balance (granted by owner or protocol)
    pub credit_balance: Amount,
    /// Maximum amount spendable within one period
    pub spending_limit: Amount,
    /// Amount already spent in the current period
    pub spent_this_period: Amount,
    /// Block height at which the current spending period started
    pub period_start_height: BlockHeight,
    /// Length of each spending period in blocks
    pub period_length: u64,
    /// Current wallet status
    pub status: WalletStatus,
    /// Block height when this wallet was created
    pub created_at: BlockHeight,
    /// Cumulative lifetime spending
    pub total_spent: Amount,
    /// Maximum credits spendable per day (~28,800 blocks at 3s)
    pub max_daily: Amount,
    /// Maximum credits spendable per month (~864,000 blocks at 3s)
    pub max_monthly: Amount,
    /// Credits spent in the current daily window
    pub daily_spent: Amount,
    /// Block height at which the current daily window started
    pub daily_reset_height: BlockHeight,
    /// Credits spent in the current monthly window
    pub monthly_spent: Amount,
    /// Block height at which the current monthly window started
    pub monthly_reset_height: BlockHeight,
    /// If Some, spending is restricted to the listed subnets; None = all subnets allowed
    pub allowed_subnets: Option<Vec<SubnetId>>,
}

/// Errors that can arise from agent wallet operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AgentWalletError {
    #[error("Wallet not found: {0}")]
    WalletNotFound(Address),

    #[error("Agent already has a wallet: {0}")]
    AgentAlreadyHasWallet(Hash),

    #[error("Unauthorized owner: {0}")]
    UnauthorizedOwner(Address),

    #[error("Wallet is frozen")]
    WalletFrozen,

    #[error("Wallet is closed")]
    WalletClosed,

    #[error("Spending limit exceeded: limit {limit}, attempted {attempted}")]
    SpendingLimitExceeded { limit: Amount, attempted: Amount },

    #[error("Insufficient balance")]
    InsufficientBalance,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Subnet not allowed for this wallet")]
    SubnetNotAllowed,

    #[error("Daily spending limit exceeded: limit {limit}, attempted {attempted}")]
    DailyLimitExceeded { limit: Amount, attempted: Amount },

    #[error("Monthly spending limit exceeded: limit {limit}, attempted {attempted}")]
    MonthlyLimitExceeded { limit: Amount, attempted: Amount },
}
