use std::collections::HashMap;
use super::types::{AdvancedToken, TokenError, TransferDetails};

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
}
