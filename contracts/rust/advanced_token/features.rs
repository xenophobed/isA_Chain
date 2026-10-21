use std::collections::HashMap;
use super::types::{AdvancedToken, TokenError, VestingSchedule};

impl AdvancedToken {
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
    
    pub(crate) fn validate_transfer(&self, from: &str, to: &str, amount: u128) -> Result<(), TokenError> {
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
        
        if let Some(max_amount) = self.max_transaction_amount {
            if amount > max_amount {
                return Err(TokenError::ExceedsMaxTransactionAmount);
            }
        }
        
        self.validate_whale_protection(to, amount)?;
        
        Ok(())
    }
    
    pub(crate) fn validate_whale_protection(&self, to: &str, amount: u128) -> Result<(), TokenError> {
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
    
    pub(crate) fn validate_address(&self, address: &str) -> Result<(), TokenError> {
        if address.is_empty() || address == "0x0" {
            return Err(TokenError::InvalidAddress {
                address: address.to_string(),
            });
        }
        Ok(())
    }
    
    pub(crate) fn calculate_transfer_amounts(&self, amount: u128) -> (u128, u128) {
        if self.transfer_fee_rate == 0 {
            return (amount, 0);
        }
        
        let fee_amount = (amount * self.transfer_fee_rate as u128) / 10000;
        let net_amount = amount - fee_amount;
        
        (net_amount, fee_amount)
    }
    
    pub(crate) fn update_balance<F>(&mut self, address: &str, f: F) -> Result<(), TokenError>
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
    
    pub(crate) fn set_allowance(&mut self, owner: &str, spender: &str, amount: u128) {
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
    
    pub(crate) fn distribute_reflections(&mut self, fee_amount: u128) {
        if fee_amount == 0 || !self.is_reflection_token {
            return;
        }
        
        let reflection_amount = (fee_amount * self.reflection_rate as u128) / 10000;
        if reflection_amount == 0 {
            return;
        }
        
        let excluded_supply: u128 = self.excluded_from_reflection
            .iter()
            .map(|addr| self.balance_of(addr))
            .sum();
        
        let eligible_supply = self.total_supply - excluded_supply;
        if eligible_supply == 0 {
            return;
        }
        
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
    
    pub(crate) fn calculate_vested_amount(&self, schedule: &VestingSchedule, current_time: u64) -> u128 {
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
