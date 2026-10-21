use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Advanced ERC20-compatible token implementation for isA_Chain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedToken {
    /// Token metadata
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub total_supply: u128,
    pub max_supply: Option<u128>,
    
    /// Balances and allowances
    pub balances: HashMap<String, u128>,
    pub allowances: HashMap<String, HashMap<String, u128>>,
    
    /// Advanced features
    pub is_mintable: bool,
    pub is_burnable: bool,
    pub is_pausable: bool,
    pub is_paused: bool,
    
    /// Access control
    pub owner: String,
    pub minters: Vec<String>,
    pub pausers: Vec<String>,
    
    /// Token economics
    pub transfer_fee_rate: u32, // Basis points (100 = 1%)
    pub fee_recipient: Option<String>,
    
    /// Governance features
    pub has_voting_power: bool,
    pub voting_snapshots: HashMap<u64, HashMap<String, u128>>, // block_height -> balances
    
    /// Compliance features
    pub whitelist_enabled: bool,
    pub whitelist: Vec<String>,
    pub blacklist: Vec<String>,
    
    /// Anti-whale measures
    pub max_wallet_percentage: Option<u32>, // Basis points (1000 = 10%)
    pub max_transaction_amount: Option<u128>,
    
    /// Reflection/dividend features
    pub is_reflection_token: bool,
    pub reflection_rate: u32, // Basis points
    pub excluded_from_reflection: Vec<String>,
    
    /// Vesting schedules
    pub vesting_schedules: HashMap<String, VestingSchedule>,
    
    /// Token metrics
    pub creation_block: u64,
    pub last_snapshot_block: u64,
}

/// Vesting schedule for token releases
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VestingSchedule {
    pub beneficiary: String,
    pub total_amount: u128,
    pub released_amount: u128,
    pub start_time: u64,
    pub cliff_duration: u64,
    pub total_duration: u64,
    pub is_revocable: bool,
    pub is_revoked: bool,
}

/// Transfer details for tracking
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferDetails {
    pub from: String,
    pub to: String,
    pub amount: u128,
    pub fee_amount: u128,
    pub block_height: u64,
    pub timestamp: u64,
}

/// Token errors
#[derive(Debug, thiserror::Error)]
pub enum TokenError {
    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance { available: u128, required: u128 },
    
    #[error("Insufficient allowance: available {available}, required {required}")]
    InsufficientAllowance { available: u128, required: u128 },
    
    #[error("Token is paused")]
    TokenPaused,
    
    #[error("Unauthorized: {action} requires {role} role")]
    Unauthorized { action: String, role: String },
    
    #[error("Address not whitelisted: {address}")]
    NotWhitelisted { address: String },
    
    #[error("Address is blacklisted: {address}")]
    Blacklisted { address: String },
    
    #[error("Exceeds maximum supply: current {current}, max {max}")]
    ExceedsMaxSupply { current: u128, max: u128 },
    
    #[error("Exceeds maximum wallet percentage")]
    ExceedsMaxWalletPercentage,
    
    #[error("Exceeds maximum transaction amount")]
    ExceedsMaxTransactionAmount,
    
    #[error("Vesting schedule not found for {address}")]
    VestingScheduleNotFound { address: String },
    
    #[error("Vesting amount not available yet")]
    VestingNotAvailable,
    
    #[error("Amount must be greater than zero")]
    InvalidAmount,
    
    #[error("Invalid address: {address}")]
    InvalidAddress { address: String },
}
