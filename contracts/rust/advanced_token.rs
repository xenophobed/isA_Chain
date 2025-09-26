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

impl AdvancedToken {
    /// Create a new advanced token
    pub fn new(
        name: String,
        symbol: String,
        decimals: u8,
        initial_supply: u128,
        owner: String,
    ) -> Self {
        let mut token = AdvancedToken {
            name,
            symbol,
            decimals,
            total_supply: initial_supply,
            max_supply: None,
            balances: HashMap::new(),
            allowances: HashMap::new(),
            is_mintable: true,
            is_burnable: true,
            is_pausable: true,
            is_paused: false,
            owner: owner.clone(),
            minters: vec![owner.clone()],
            pausers: vec![owner.clone()],
            transfer_fee_rate: 0,
            fee_recipient: None,
            has_voting_power: false,
            voting_snapshots: HashMap::new(),
            whitelist_enabled: false,
            whitelist: Vec::new(),
            blacklist: Vec::new(),
            max_wallet_percentage: None,
            max_transaction_amount: None,
            is_reflection_token: false,
            reflection_rate: 0,
            excluded_from_reflection: Vec::new(),
            vesting_schedules: HashMap::new(),
            creation_block: 0,
            last_snapshot_block: 0,
        };
        
        // Give initial supply to owner
        if initial_supply > 0 {
            token.balances.insert(owner, initial_supply);
        }
        
        token
    }
    
    /// Get balance of an address
    pub fn balance_of(&self, address: &str) -> u128 {
        *self.balances.get(address).unwrap_or(&0)
    }
    
    /// Get allowance amount
    pub fn allowance(&self, owner: &str, spender: &str) -> u128 {
        self.allowances
            .get(owner)
            .and_then(|allowances| allowances.get(spender))
            .copied()
            .unwrap_or(0)
    }
    
    /// Transfer tokens
    pub fn transfer(
        &mut self,
        from: &str,
        to: &str,
        amount: u128,
        current_block: u64,
        current_time: u64,
    ) -> Result<TransferDetails, TokenError> {
        self.validate_transfer(from, to, amount)?;
        
        let (net_amount, fee_amount) = self.calculate_transfer_amounts(amount);
        
        // Update balances
        self.update_balance(from, |balance| balance - amount)?;
        self.update_balance(to, |balance| balance + net_amount)?;
        
        // Handle fees
        if fee_amount > 0 {
            if let Some(fee_recipient) = &self.fee_recipient {
                self.update_balance(fee_recipient, |balance| balance + fee_amount)?;
            }
        }
        
        // Handle reflections
        if self.is_reflection_token {
            self.distribute_reflections(fee_amount);
        }
        
        Ok(TransferDetails {
            from: from.to_string(),
            to: to.to_string(),
            amount,
            fee_amount,
            block_height: current_block,
            timestamp: current_time,
        })
    }
    
    /// Transfer from (allowance-based)
    pub fn transfer_from(
        &mut self,
        spender: &str,
        from: &str,
        to: &str,
        amount: u128,
        current_block: u64,
        current_time: u64,
    ) -> Result<TransferDetails, TokenError> {
        let current_allowance = self.allowance(from, spender);
        if current_allowance < amount {
            return Err(TokenError::InsufficientAllowance {
                available: current_allowance,
                required: amount,
            });
        }
        
        // Update allowance
        self.set_allowance(from, spender, current_allowance - amount);
        
        // Perform transfer
        self.transfer(from, to, amount, current_block, current_time)
    }
    
    /// Approve spender
    pub fn approve(&mut self, owner: &str, spender: &str, amount: u128) -> Result<(), TokenError> {
        if self.is_paused {
            return Err(TokenError::TokenPaused);
        }
        
        self.validate_address(owner)?;
        self.validate_address(spender)?;
        
        self.set_allowance(owner, spender, amount);
        Ok(())
    }
    
    /// Mint new tokens
    pub fn mint(&mut self, to: &str, amount: u128, minter: &str) -> Result<(), TokenError> {
        if !self.is_mintable {
            return Err(TokenError::Unauthorized {
                action: "minting".to_string(),
                role: "mintable token".to_string(),
            });
        }
        
        if !self.minters.contains(&minter.to_string()) {
            return Err(TokenError::Unauthorized {
                action: "minting".to_string(),
                role: "minter".to_string(),
            });
        }
        
        if self.is_paused {
            return Err(TokenError::TokenPaused);
        }
        
        // Check max supply
        if let Some(max_supply) = self.max_supply {
            if self.total_supply + amount > max_supply {
                return Err(TokenError::ExceedsMaxSupply {
                    current: self.total_supply + amount,
                    max: max_supply,
                });
            }
        }
        
        self.validate_address(to)?;
        self.validate_whale_protection(to, amount)?;
        
        self.total_supply += amount;
        self.update_balance(to, |balance| balance + amount)?;
        
        Ok(())
    }
    
    /// Burn tokens
    pub fn burn(&mut self, from: &str, amount: u128, burner: &str) -> Result<(), TokenError> {
        if !self.is_burnable {
            return Err(TokenError::Unauthorized {
                action: "burning".to_string(),
                role: "burnable token".to_string(),
            });
        }
        
        // Allow self-burning or authorized burners
        if from != burner && !self.minters.contains(&burner.to_string()) {
            return Err(TokenError::Unauthorized {
                action: "burning".to_string(),
                role: "token owner or burner".to_string(),
            });
        }
        
        if self.is_paused {
            return Err(TokenError::TokenPaused);
        }
        
        let current_balance = self.balance_of(from);
        if current_balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: current_balance,
                required: amount,
            });
        }
        
        self.total_supply -= amount;
        self.update_balance(from, |balance| balance - amount)?;
        
        Ok(())
    }
    
    /// Create vesting schedule
    pub fn create_vesting_schedule(
        &mut self,
        beneficiary: String,
        total_amount: u128,
        start_time: u64,
        cliff_duration: u64,
        total_duration: u64,
        is_revocable: bool,
        grantor: &str,
    ) -> Result<(), TokenError> {
        if grantor != self.owner {
            return Err(TokenError::Unauthorized {
                action: "creating vesting schedule".to_string(),
                role: "owner".to_string(),
            });
        }
        
        if self.vesting_schedules.contains_key(&beneficiary) {
            return Err(TokenError::Unauthorized {
                action: "creating vesting schedule".to_string(),
                role: "unique beneficiary".to_string(),
            });
        }
        
        // Transfer tokens to contract (simulate)
        let grantor_balance = self.balance_of(grantor);
        if grantor_balance < total_amount {
            return Err(TokenError::InsufficientBalance {
                available: grantor_balance,
                required: total_amount,
            });
        }
        
        self.update_balance(grantor, |balance| balance - total_amount)?;
        
        let schedule = VestingSchedule {
            beneficiary: beneficiary.clone(),
            total_amount,
            released_amount: 0,
            start_time,
            cliff_duration,
            total_duration,
            is_revocable,
            is_revoked: false,
        };
        
        self.vesting_schedules.insert(beneficiary, schedule);
        Ok(())
    }
    
    /// Release vested tokens
    pub fn release_vested_tokens(
        &mut self,
        beneficiary: &str,
        current_time: u64,
    ) -> Result<u128, TokenError> {
        let schedule = self.vesting_schedules
            .get_mut(beneficiary)
            .ok_or_else(|| TokenError::VestingScheduleNotFound {
                address: beneficiary.to_string(),
            })?;
        
        if schedule.is_revoked {
            return Err(TokenError::VestingNotAvailable);
        }
        
        let vested_amount = self.calculate_vested_amount(schedule, current_time);
        let releasable_amount = vested_amount - schedule.released_amount;
        
        if releasable_amount == 0 {
            return Err(TokenError::VestingNotAvailable);
        }
        
        schedule.released_amount += releasable_amount;
        self.update_balance(beneficiary, |balance| balance + releasable_amount)?;
        
        Ok(releasable_amount)
    }
    
    /// Take voting snapshot
    pub fn take_snapshot(&mut self, block_height: u64) -> Result<(), TokenError> {
        if !self.has_voting_power {
            return Err(TokenError::Unauthorized {
                action: "taking snapshot".to_string(),
                role: "governance token".to_string(),
            });
        }
        
        self.voting_snapshots.insert(block_height, self.balances.clone());
        self.last_snapshot_block = block_height;
        
        Ok(())
    }
    
    /// Get voting power at specific block
    pub fn get_voting_power(&self, address: &str, block_height: u64) -> u128 {
        if !self.has_voting_power {
            return 0;
        }
        
        // Find the snapshot at or before the requested block height
        let mut best_snapshot_block = 0;
        for &snapshot_block in self.voting_snapshots.keys() {
            if snapshot_block <= block_height && snapshot_block > best_snapshot_block {
                best_snapshot_block = snapshot_block;
            }
        }
        
        if best_snapshot_block == 0 {
            return 0;
        }
        
        self.voting_snapshots
            .get(&best_snapshot_block)
            .and_then(|snapshot| snapshot.get(address))
            .copied()
            .unwrap_or(0)
    }
    
    /// Pause token transfers
    pub fn pause(&mut self, pauser: &str) -> Result<(), TokenError> {
        if !self.is_pausable {
            return Err(TokenError::Unauthorized {
                action: "pausing".to_string(),
                role: "pausable token".to_string(),
            });
        }
        
        if !self.pausers.contains(&pauser.to_string()) {
            return Err(TokenError::Unauthorized {
                action: "pausing".to_string(),
                role: "pauser".to_string(),
            });
        }
        
        self.is_paused = true;
        Ok(())
    }
    
    /// Unpause token transfers
    pub fn unpause(&mut self, pauser: &str) -> Result<(), TokenError> {
        if !self.pausers.contains(&pauser.to_string()) {
            return Err(TokenError::Unauthorized {
                action: "unpausing".to_string(),
                role: "pauser".to_string(),
            });
        }
        
        self.is_paused = false;
        Ok(())
    }
    
    // Helper methods
    
    fn validate_transfer(&self, from: &str, to: &str, amount: u128) -> Result<(), TokenError> {
        if self.is_paused {
            return Err(TokenError::TokenPaused);
        }
        
        if amount == 0 {
            return Err(TokenError::InvalidAmount);
        }
        
        self.validate_address(from)?;
        self.validate_address(to)?;
        
        let from_balance = self.balance_of(from);
        if from_balance < amount {
            return Err(TokenError::InsufficientBalance {
                available: from_balance,
                required: amount,
            });
        }
        
        // Check compliance
        if self.whitelist_enabled && !self.whitelist.contains(&to.to_string()) {
            return Err(TokenError::NotWhitelisted {
                address: to.to_string(),
            });
        }
        
        if self.blacklist.contains(&from.to_string()) || self.blacklist.contains(&to.to_string()) {
            return Err(TokenError::Blacklisted {
                address: if self.blacklist.contains(&from.to_string()) {
                    from.to_string()
                } else {
                    to.to_string()
                },
            });
        }
        
        // Check transaction limits
        if let Some(max_amount) = self.max_transaction_amount {
            if amount > max_amount {
                return Err(TokenError::ExceedsMaxTransactionAmount);
            }
        }
        
        self.validate_whale_protection(to, amount)?;
        
        Ok(())
    }
    
    fn validate_whale_protection(&self, to: &str, amount: u128) -> Result<(), TokenError> {
        if let Some(max_percentage) = self.max_wallet_percentage {
            let current_balance = self.balance_of(to);
            let new_balance = current_balance + amount;
            let max_allowed = (self.total_supply * max_percentage as u128) / 10000;
            
            if new_balance > max_allowed {
                return Err(TokenError::ExceedsMaxWalletPercentage);
            }
        }
        
        Ok(())
    }
    
    fn validate_address(&self, address: &str) -> Result<(), TokenError> {
        if address.is_empty() || address == "0x0" {
            return Err(TokenError::InvalidAddress {
                address: address.to_string(),
            });
        }
        Ok(())
    }
    
    fn calculate_transfer_amounts(&self, amount: u128) -> (u128, u128) {
        if self.transfer_fee_rate == 0 {
            return (amount, 0);
        }
        
        let fee_amount = (amount * self.transfer_fee_rate as u128) / 10000;
        let net_amount = amount - fee_amount;
        
        (net_amount, fee_amount)
    }
    
    fn update_balance<F>(&mut self, address: &str, f: F) -> Result<(), TokenError>
    where
        F: FnOnce(u128) -> u128,
    {
        let current_balance = self.balance_of(address);
        let new_balance = f(current_balance);
        
        if new_balance == 0 {
            self.balances.remove(address);
        } else {
            self.balances.insert(address.to_string(), new_balance);
        }
        
        Ok(())
    }
    
    fn set_allowance(&mut self, owner: &str, spender: &str, amount: u128) {
        if amount == 0 {
            if let Some(owner_allowances) = self.allowances.get_mut(owner) {
                owner_allowances.remove(spender);
                if owner_allowances.is_empty() {
                    self.allowances.remove(owner);
                }
            }
        } else {
            self.allowances
                .entry(owner.to_string())
                .or_insert_with(HashMap::new)
                .insert(spender.to_string(), amount);
        }
    }
    
    fn distribute_reflections(&mut self, fee_amount: u128) {
        if fee_amount == 0 || !self.is_reflection_token {
            return;
        }
        
        let reflection_amount = (fee_amount * self.reflection_rate as u128) / 10000;
        if reflection_amount == 0 {
            return;
        }
        
        // Calculate total supply excluding excluded addresses
        let excluded_supply: u128 = self.excluded_from_reflection
            .iter()
            .map(|addr| self.balance_of(addr))
            .sum();
        
        let eligible_supply = self.total_supply - excluded_supply;
        if eligible_supply == 0 {
            return;
        }
        
        // Distribute reflections proportionally
        let balances_clone = self.balances.clone();
        for (address, balance) in balances_clone {
            if !self.excluded_from_reflection.contains(&address) && balance > 0 {
                let reflection_share = (reflection_amount * balance) / eligible_supply;
                if reflection_share > 0 {
                    let _ = self.update_balance(&address, |b| b + reflection_share);
                }
            }
        }
    }
    
    fn calculate_vested_amount(&self, schedule: &VestingSchedule, current_time: u64) -> u128 {
        if current_time < schedule.start_time + schedule.cliff_duration {
            return 0;
        }
        
        if current_time >= schedule.start_time + schedule.total_duration {
            return schedule.total_amount;
        }
        
        let elapsed_time = current_time - schedule.start_time;
        (schedule.total_amount * elapsed_time as u128) / schedule.total_duration as u128
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_token_creation() {
        let token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        assert_eq!(token.name, "Test Token");
        assert_eq!(token.symbol, "TEST");
        assert_eq!(token.total_supply, 1000000);
        assert_eq!(token.balance_of("owner"), 1000000);
    }
    
    #[test]
    fn test_transfer() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        let result = token.transfer("owner", "user1", 100000, 1, 1000);
        assert!(result.is_ok());
        
        assert_eq!(token.balance_of("owner"), 900000);
        assert_eq!(token.balance_of("user1"), 100000);
    }
    
    #[test]
    fn test_transfer_with_fee() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        token.transfer_fee_rate = 100; // 1%
        token.fee_recipient = Some("fee_collector".to_string());
        
        let result = token.transfer("owner", "user1", 100000, 1, 1000);
        assert!(result.is_ok());
        
        assert_eq!(token.balance_of("owner"), 900000);
        assert_eq!(token.balance_of("user1"), 99000); // 100000 - 1000 fee
        assert_eq!(token.balance_of("fee_collector"), 1000);
    }
    
    #[test]
    fn test_minting() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        let result = token.mint("user1", 50000, "owner");
        assert!(result.is_ok());
        
        assert_eq!(token.total_supply, 1050000);
        assert_eq!(token.balance_of("user1"), 50000);
    }
    
    #[test]
    fn test_burning() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        let result = token.burn("owner", 50000, "owner");
        assert!(result.is_ok());
        
        assert_eq!(token.total_supply, 950000);
        assert_eq!(token.balance_of("owner"), 950000);
    }
    
    #[test]
    fn test_approval_and_transfer_from() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        // Approve spender
        let approve_result = token.approve("owner", "spender", 100000);
        assert!(approve_result.is_ok());
        assert_eq!(token.allowance("owner", "spender"), 100000);
        
        // Transfer from
        let transfer_result = token.transfer_from("spender", "owner", "user1", 50000, 1, 1000);
        assert!(transfer_result.is_ok());
        
        assert_eq!(token.balance_of("owner"), 950000);
        assert_eq!(token.balance_of("user1"), 50000);
        assert_eq!(token.allowance("owner", "spender"), 50000);
    }
    
    #[test]
    fn test_pause_functionality() {
        let mut token = AdvancedToken::new(
            "Test Token".to_string(),
            "TEST".to_string(),
            18,
            1000000,
            "owner".to_string(),
        );
        
        // Pause token
        let pause_result = token.pause("owner");
        assert!(pause_result.is_ok());
        assert!(token.is_paused);
        
        // Try to transfer while paused
        let transfer_result = token.transfer("owner", "user1", 100000, 1, 1000);
        assert!(transfer_result.is_err());
        
        // Unpause token
        let unpause_result = token.unpause("owner");
        assert!(unpause_result.is_ok());
        assert!(!token.is_paused);
        
        // Transfer should work now
        let transfer_result = token.transfer("owner", "user1", 100000, 1, 1000);
        assert!(transfer_result.is_ok());
    }
}