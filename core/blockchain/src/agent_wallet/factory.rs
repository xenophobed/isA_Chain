use std::collections::HashMap;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};
use super::types::*;

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
    #[allow(clippy::too_many_arguments)]
    pub fn create_wallet(
        &mut self,
        agent_id: Hash,
        owner: Address,
        spending_limit: Amount,
        period_length: u64,
        height: BlockHeight,
        max_daily: Amount,
        max_monthly: Amount,
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
            max_daily,
            max_monthly,
            daily_spent: 0,
            daily_reset_height: height,
            monthly_spent: 0,
            monthly_reset_height: height,
            allowed_subnets: None,
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
    /// Automatically resets the spending period, daily window, and monthly window
    /// if they have expired. Checks wallet status, spending limit, daily limit,
    /// monthly limit, and balance. If `subnet` is provided and `allowed_subnets`
    /// is Some, also checks subnet membership.
    pub fn spend(
        &mut self,
        wallet_address: &Address,
        amount: Amount,
        current_height: BlockHeight,
        subnet: Option<SubnetId>,
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

        // Check subnet allowlist
        if let (Some(allowed), Some(ref requested)) = (&wallet.allowed_subnets, &subnet) {
            if !allowed.contains(requested) {
                return Err(AgentWalletError::SubnetNotAllowed);
            }
        }

        // Auto-reset period limit if expired
        if current_height >= wallet.period_start_height + wallet.period_length {
            wallet.spent_this_period = 0;
            wallet.period_start_height = current_height;
        }

        // Auto-reset daily limit if expired
        if current_height >= wallet.daily_reset_height + DAILY_BLOCKS {
            wallet.daily_spent = 0;
            wallet.daily_reset_height = current_height;
        }

        // Auto-reset monthly limit if expired
        if current_height >= wallet.monthly_reset_height + MONTHLY_BLOCKS {
            wallet.monthly_spent = 0;
            wallet.monthly_reset_height = current_height;
        }

        // Check period spending limit
        let new_period_spent = wallet
            .spent_this_period
            .checked_add(amount)
            .unwrap_or(Amount::MAX);
        if new_period_spent > wallet.spending_limit {
            return Err(AgentWalletError::SpendingLimitExceeded {
                limit: wallet.spending_limit,
                attempted: wallet.spent_this_period + amount,
            });
        }

        // Check daily limit (0 means unlimited)
        if wallet.max_daily > 0 {
            let new_daily = wallet
                .daily_spent
                .checked_add(amount)
                .unwrap_or(Amount::MAX);
            if new_daily > wallet.max_daily {
                return Err(AgentWalletError::DailyLimitExceeded {
                    limit: wallet.max_daily,
                    attempted: wallet.daily_spent + amount,
                });
            }
        }

        // Check monthly limit (0 means unlimited)
        if wallet.max_monthly > 0 {
            let new_monthly = wallet
                .monthly_spent
                .checked_add(amount)
                .unwrap_or(Amount::MAX);
            if new_monthly > wallet.max_monthly {
                return Err(AgentWalletError::MonthlyLimitExceeded {
                    limit: wallet.max_monthly,
                    attempted: wallet.monthly_spent + amount,
                });
            }
        }

        // Check balance
        if wallet.balance < amount {
            return Err(AgentWalletError::InsufficientBalance);
        }

        wallet.balance -= amount;
        wallet.spent_this_period = new_period_spent;
        wallet.daily_spent += amount;
        wallet.monthly_spent += amount;
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
