use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
}

/// Factory for creating and managing agent-specific wallets
pub struct AgentWalletFactory {
    /// All wallets keyed by wallet address
    pub wallets: HashMap<Address, AgentWallet>,
    /// Index: owner address → list of wallet addresses
    pub wallets_by_owner: HashMap<Address, Vec<Address>>,
    /// Index: agent_id → wallet address
    pub wallets_by_agent: HashMap<Hash, Address>,
    /// Default spending limit applied when creating wallets
    pub default_spending_limit: Amount,
    /// Default period length (in blocks) applied when creating wallets
    pub default_period_length: u64,
}

impl AgentWalletFactory {
    /// Create a new factory with given defaults.
    pub fn new(default_spending_limit: Amount, default_period_length: u64) -> Self {
        AgentWalletFactory {
            wallets: HashMap::new(),
            wallets_by_owner: HashMap::new(),
            wallets_by_agent: HashMap::new(),
            default_spending_limit,
            default_period_length,
        }
    }

    /// Create a new agent wallet. The wallet address is derived deterministically
    /// from the `agent_id`.
    ///
    /// Returns the newly created wallet's address, or an error if the agent
    /// already has a wallet.
    pub fn create_wallet(
        &mut self,
        agent_id: Hash,
        owner: Address,
        spending_limit: Amount,
        period_length: u64,
        height: BlockHeight,
    ) -> Result<Address, AgentWalletError> {
        if self.wallets_by_agent.contains_key(&agent_id) {
            return Err(AgentWalletError::AgentAlreadyHasWallet(agent_id));
        }

        let address = Address::from_public_key(agent_id.as_bytes());

        let wallet = AgentWallet {
            address,
            agent_id,
            owner,
            balance: 0,
            credit_balance: 0,
            spending_limit,
            spent_this_period: 0,
            period_start_height: height,
            period_length,
            status: WalletStatus::Active,
            created_at: height,
            total_spent: 0,
        };

        self.wallets.insert(address, wallet);
        self.wallets_by_agent.insert(agent_id, address);
        self.wallets_by_owner
            .entry(owner)
            .or_default()
            .push(address);

        Ok(address)
    }

    /// Look up a wallet by address.
    pub fn get_wallet(&self, address: &Address) -> Option<&AgentWallet> {
        self.wallets.get(address)
    }

    /// Look up a wallet mutably by address.
    pub fn get_wallet_mut(&mut self, address: &Address) -> Option<&mut AgentWallet> {
        self.wallets.get_mut(address)
    }

    /// Look up a wallet by the agent's unique identifier.
    pub fn get_wallet_by_agent(&self, agent_id: &Hash) -> Option<&AgentWallet> {
        let address = self.wallets_by_agent.get(agent_id)?;
        self.wallets.get(address)
    }

    /// Return all wallets belonging to a given owner.
    pub fn get_wallets_by_owner(&self, owner: &Address) -> Vec<&AgentWallet> {
        match self.wallets_by_owner.get(owner) {
            Some(addresses) => addresses
                .iter()
                .filter_map(|addr| self.wallets.get(addr))
                .collect(),
            None => Vec::new(),
        }
    }

    /// Deposit funds into a wallet.
    pub fn deposit(
        &mut self,
        wallet_address: &Address,
        amount: Amount,
    ) -> Result<(), AgentWalletError> {
        if amount == 0 {
            return Err(AgentWalletError::InvalidAmount);
        }
        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        if wallet.status == WalletStatus::Closed {
            return Err(AgentWalletError::WalletClosed);
        }

        wallet.balance += amount;
        Ok(())
    }

    /// Spend `amount` from a wallet.
    ///
    /// Automatically resets the spending period if it has expired.
    /// Checks wallet status, spending limit, and balance.
    pub fn spend(
        &mut self,
        wallet_address: &Address,
        amount: Amount,
        current_height: BlockHeight,
    ) -> Result<(), AgentWalletError> {
        if amount == 0 {
            return Err(AgentWalletError::InvalidAmount);
        }

        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        match wallet.status {
            WalletStatus::Frozen => return Err(AgentWalletError::WalletFrozen),
            WalletStatus::Closed => return Err(AgentWalletError::WalletClosed),
            WalletStatus::Suspended => return Err(AgentWalletError::WalletFrozen),
            WalletStatus::Active => {}
        }

        // Auto-reset spending period if expired
        if current_height >= wallet.period_start_height + wallet.period_length {
            wallet.spent_this_period = 0;
            wallet.period_start_height = current_height;
        }

        // Check spending limit
        let new_spent = wallet
            .spent_this_period
            .checked_add(amount)
            .unwrap_or(Amount::MAX);
        if new_spent > wallet.spending_limit {
            return Err(AgentWalletError::SpendingLimitExceeded {
                limit: wallet.spending_limit,
                attempted: wallet.spent_this_period + amount,
            });
        }

        // Check balance
        if wallet.balance < amount {
            return Err(AgentWalletError::InsufficientBalance);
        }

        wallet.balance -= amount;
        wallet.spent_this_period = new_spent;
        wallet.total_spent += amount;
        Ok(())
    }

    /// Freeze a wallet (owner-only).
    pub fn freeze(
        &mut self,
        wallet_address: &Address,
        owner: &Address,
    ) -> Result<(), AgentWalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        if &wallet.owner != owner {
            return Err(AgentWalletError::UnauthorizedOwner(*owner));
        }
        if wallet.status == WalletStatus::Closed {
            return Err(AgentWalletError::WalletClosed);
        }

        wallet.status = WalletStatus::Frozen;
        Ok(())
    }

    /// Unfreeze a previously frozen wallet (owner-only).
    pub fn unfreeze(
        &mut self,
        wallet_address: &Address,
        owner: &Address,
    ) -> Result<(), AgentWalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        if &wallet.owner != owner {
            return Err(AgentWalletError::UnauthorizedOwner(*owner));
        }
        if wallet.status == WalletStatus::Closed {
            return Err(AgentWalletError::WalletClosed);
        }

        wallet.status = WalletStatus::Active;
        Ok(())
    }

    /// Close a wallet and return its remaining balance to the caller (owner-only).
    pub fn close(
        &mut self,
        wallet_address: &Address,
        owner: &Address,
    ) -> Result<Amount, AgentWalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        if &wallet.owner != owner {
            return Err(AgentWalletError::UnauthorizedOwner(*owner));
        }
        if wallet.status == WalletStatus::Closed {
            return Err(AgentWalletError::WalletClosed);
        }

        let remaining = wallet.balance;
        wallet.balance = 0;
        wallet.status = WalletStatus::Closed;
        Ok(remaining)
    }

    /// Update the spending limit on a wallet (owner-only).
    pub fn set_spending_limit(
        &mut self,
        wallet_address: &Address,
        new_limit: Amount,
        owner: &Address,
    ) -> Result<(), AgentWalletError> {
        let wallet = self
            .wallets
            .get_mut(wallet_address)
            .ok_or(AgentWalletError::WalletNotFound(*wallet_address))?;

        if &wallet.owner != owner {
            return Err(AgentWalletError::UnauthorizedOwner(*owner));
        }
        if wallet.status == WalletStatus::Closed {
            return Err(AgentWalletError::WalletClosed);
        }

        wallet.spending_limit = new_limit;
        Ok(())
    }

    /// Reset the spending period for a wallet if the current period has expired.
    ///
    /// This is a no-op if the period has not yet expired.
    pub fn reset_period(&mut self, wallet_address: &Address, current_height: BlockHeight) {
        if let Some(wallet) = self.wallets.get_mut(wallet_address) {
            if current_height >= wallet.period_start_height + wallet.period_length {
                wallet.spent_this_period = 0;
                wallet.period_start_height = current_height;
            }
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 1 ISA in wei
    const ONE_ISA: Amount = 1_000_000_000_000_000_000;
    const DEFAULT_PERIOD: u64 = 1000;

    fn make_factory() -> AgentWalletFactory {
        AgentWalletFactory::new(ONE_ISA, DEFAULT_PERIOD)
    }

    fn make_agent_id(seed: u8) -> Hash {
        Hash::new([seed; 32])
    }

    fn make_owner(seed: u8) -> Address {
        Address::from([seed; 20])
    }

    // -----------------------------------------------------------------------
    // Wallet creation
    // -----------------------------------------------------------------------

    #[test]
    fn test_create_wallet() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(1);
        let owner = make_owner(0xAA);

        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .expect("should create wallet");

        let wallet = factory.get_wallet(&addr).expect("wallet should exist");
        assert_eq!(wallet.agent_id, agent_id);
        assert_eq!(wallet.owner, owner);
        assert_eq!(wallet.balance, 0);
        assert_eq!(wallet.spending_limit, ONE_ISA);
        assert_eq!(wallet.period_length, DEFAULT_PERIOD);
        assert_eq!(wallet.status, WalletStatus::Active);
        assert_eq!(wallet.created_at, 0);

        // Deterministic address derivation
        let expected_addr = Address::from_public_key(agent_id.as_bytes());
        assert_eq!(addr, expected_addr);
    }

    #[test]
    fn test_duplicate_agent_fails() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(2);
        let owner = make_owner(0xBB);

        factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        let result = factory.create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 1);
        assert_eq!(result, Err(AgentWalletError::AgentAlreadyHasWallet(agent_id)));
    }

    // -----------------------------------------------------------------------
    // Deposit
    // -----------------------------------------------------------------------

    #[test]
    fn test_deposit() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(3);
        let owner = make_owner(0xCC);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.deposit(&addr, ONE_ISA).unwrap();
        let wallet = factory.get_wallet(&addr).unwrap();
        assert_eq!(wallet.balance, ONE_ISA);

        // Deposit again
        factory.deposit(&addr, 500).unwrap();
        assert_eq!(factory.get_wallet(&addr).unwrap().balance, ONE_ISA + 500);
    }

    // -----------------------------------------------------------------------
    // Spend — success path
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_success() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(4);
        let owner = make_owner(0xDD);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.deposit(&addr, ONE_ISA).unwrap();
        factory.spend(&addr, ONE_ISA / 2, 0).unwrap();

        let wallet = factory.get_wallet(&addr).unwrap();
        assert_eq!(wallet.balance, ONE_ISA / 2);
        assert_eq!(wallet.spent_this_period, ONE_ISA / 2);
        assert_eq!(wallet.total_spent, ONE_ISA / 2);
    }

    // -----------------------------------------------------------------------
    // Spend — limit exceeded
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_exceeds_limit() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(5);
        let owner = make_owner(0xEE);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        // Fund more than the limit so balance isn't the constraint
        factory.deposit(&addr, ONE_ISA * 10).unwrap();

        let result = factory.spend(&addr, ONE_ISA + 1, 0);
        assert!(matches!(
            result,
            Err(AgentWalletError::SpendingLimitExceeded { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // Spend — insufficient balance
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_insufficient_balance() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(6);
        let owner = make_owner(0xFF);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        // Deposit less than we try to spend
        factory.deposit(&addr, 100).unwrap();

        let result = factory.spend(&addr, 200, 0);
        assert_eq!(result, Err(AgentWalletError::InsufficientBalance));
    }

    // -----------------------------------------------------------------------
    // Spend — frozen wallet
    // -----------------------------------------------------------------------

    #[test]
    fn test_spend_frozen_fails() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(7);
        let owner = make_owner(0x11);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.deposit(&addr, ONE_ISA).unwrap();
        factory.freeze(&addr, &owner).unwrap();

        let result = factory.spend(&addr, 100, 0);
        assert_eq!(result, Err(AgentWalletError::WalletFrozen));
    }

    // -----------------------------------------------------------------------
    // Freeze / unfreeze
    // -----------------------------------------------------------------------

    #[test]
    fn test_freeze_unfreeze() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(8);
        let owner = make_owner(0x22);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.freeze(&addr, &owner).unwrap();
        assert_eq!(
            factory.get_wallet(&addr).unwrap().status,
            WalletStatus::Frozen
        );

        factory.unfreeze(&addr, &owner).unwrap();
        assert_eq!(
            factory.get_wallet(&addr).unwrap().status,
            WalletStatus::Active
        );
    }

    // -----------------------------------------------------------------------
    // Close
    // -----------------------------------------------------------------------

    #[test]
    fn test_close_returns_balance() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(9);
        let owner = make_owner(0x33);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.deposit(&addr, ONE_ISA).unwrap();
        let returned = factory.close(&addr, &owner).unwrap();

        assert_eq!(returned, ONE_ISA);
        assert_eq!(factory.get_wallet(&addr).unwrap().balance, 0);
        assert_eq!(
            factory.get_wallet(&addr).unwrap().status,
            WalletStatus::Closed
        );

        // Closing again should fail
        let result = factory.close(&addr, &owner);
        assert_eq!(result, Err(AgentWalletError::WalletClosed));
    }

    // -----------------------------------------------------------------------
    // Set spending limit
    // -----------------------------------------------------------------------

    #[test]
    fn test_set_spending_limit() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(10);
        let owner = make_owner(0x44);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.set_spending_limit(&addr, ONE_ISA * 5, &owner).unwrap();
        assert_eq!(
            factory.get_wallet(&addr).unwrap().spending_limit,
            ONE_ISA * 5
        );
    }

    // -----------------------------------------------------------------------
    // Period reset
    // -----------------------------------------------------------------------

    #[test]
    fn test_period_reset() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(11);
        let owner = make_owner(0x55);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        factory.deposit(&addr, ONE_ISA * 10).unwrap();

        // Spend up to the limit in period 0
        factory.spend(&addr, ONE_ISA, 0).unwrap();
        assert_eq!(
            factory.get_wallet(&addr).unwrap().spent_this_period,
            ONE_ISA
        );

        // Advance past the period boundary — spend should auto-reset
        let next_period_height = DEFAULT_PERIOD; // period_start(0) + period_length(1000) = 1000
        factory.spend(&addr, ONE_ISA / 2, next_period_height).unwrap();

        let wallet = factory.get_wallet(&addr).unwrap();
        assert_eq!(wallet.spent_this_period, ONE_ISA / 2);
        assert_eq!(wallet.period_start_height, next_period_height);
    }

    // -----------------------------------------------------------------------
    // Unauthorized owner
    // -----------------------------------------------------------------------

    #[test]
    fn test_unauthorized_owner_fails() {
        let mut factory = make_factory();
        let agent_id = make_agent_id(12);
        let owner = make_owner(0x66);
        let intruder = make_owner(0x77);
        let addr = factory
            .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        assert_eq!(
            factory.freeze(&addr, &intruder),
            Err(AgentWalletError::UnauthorizedOwner(intruder))
        );
        assert_eq!(
            factory.close(&addr, &intruder),
            Err(AgentWalletError::UnauthorizedOwner(intruder))
        );
        assert_eq!(
            factory.set_spending_limit(&addr, 0, &intruder),
            Err(AgentWalletError::UnauthorizedOwner(intruder))
        );
    }

    // -----------------------------------------------------------------------
    // Get wallets by owner
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_wallets_by_owner() {
        let mut factory = make_factory();
        let owner = make_owner(0x88);

        // Create two wallets for the same owner
        let addr1 = factory
            .create_wallet(make_agent_id(20), owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();
        let addr2 = factory
            .create_wallet(make_agent_id(21), owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        // Different owner — should not appear
        let other_owner = make_owner(0x99);
        factory
            .create_wallet(make_agent_id(22), other_owner, ONE_ISA, DEFAULT_PERIOD, 0)
            .unwrap();

        let wallets = factory.get_wallets_by_owner(&owner);
        assert_eq!(wallets.len(), 2);
        let addresses: Vec<Address> = wallets.iter().map(|w| w.address).collect();
        assert!(addresses.contains(&addr1));
        assert!(addresses.contains(&addr2));

        // Owner with no wallets
        let empty_owner = make_owner(0xAB);
        assert!(factory.get_wallets_by_owner(&empty_owner).is_empty());
    }
}
