use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};

// ====================================================================
// Constants
// ====================================================================

/// Default credit price: $0.00001 USD = 100 micro-USD
pub const DEFAULT_CREDIT_PRICE_USD: Amount = 100;

/// Default minimum purchase: 100 credits = $0.001 worth
pub const DEFAULT_MIN_PURCHASE: Amount = 100;


// ====================================================================
// Errors
// ====================================================================

/// Errors related to credit system operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditError {
    #[error("Insufficient credits: not enough credits to complete operation")]
    InsufficientCredits,

    #[error("Insufficient ISA: not enough ISA tokens to purchase credits")]
    InsufficientISA,

    #[error("Below minimum purchase: must purchase at least the minimum credit amount")]
    BelowMinimumPurchase,

    #[error("Invalid amount: amount must be greater than zero")]
    InvalidAmount,

    #[error("Account not found: {0}")]
    AccountNotFound(Address),

    #[error("Unauthorized admin: {0} is not the admin")]
    UnauthorizedAdmin(Address),
}

// ====================================================================
// CreditAccount
// ====================================================================

/// An account in the ISA Credits system.
///
/// Credits are the stable payment unit for platform services, pegged to USD.
/// 1 credit = $0.01 USD = 1_000_000 micro-credits in the base unit.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditAccount {
    /// Account address
    pub address: Address,
    /// Balance in micro-credits (1_000_000 = 1 credit = $0.01 USD)
    pub credit_balance: Amount,
    /// Cumulative credits purchased over account lifetime
    pub total_credits_purchased: Amount,
    /// Cumulative credits spent over account lifetime
    pub total_credits_spent: Amount,
    /// Block height of the most recent top-up
    pub last_top_up_height: BlockHeight,
}

impl CreditAccount {
    pub(crate) fn new(address: Address) -> Self {
        CreditAccount {
            address,
            credit_balance: 0,
            total_credits_purchased: 0,
            total_credits_spent: 0,
            last_top_up_height: 0,
        }
    }
}
