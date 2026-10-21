use super::types::*;
use crate::staking::StakingVault;
use crate::types::{Address, Amount, BlockHeight};

/// Proof-of-Stake consensus engine.
///
/// Wraps a [`StakingVault`] and maintains the active validator set.
/// Block proposers are selected via weighted round-robin keyed on height.
pub struct PoSConsensus {
    /// Staking vault that holds validator collateral
    pub staking_vault: StakingVault,
    /// Active validator set, ordered by registration time
    pub validators: Vec<ValidatorInfo>,
    /// Currently selected block proposer (updated by `select_proposer`)
    pub current_proposer: Option<Address>,
    /// Number of blocks per epoch
    pub epoch_length: u64,
    /// Minimum number of validators required before consensus can proceed
    pub min_validators: usize,
}

impl PoSConsensus {
    // ------------------------------------------------------------------
    // Constructor
    // ------------------------------------------------------------------

    /// Create a new PoS engine.
    ///
    /// * `epoch_length`    — blocks per epoch (default recommended: 100)
    /// * `min_validators`  — minimum active validators required (default: 1)
    pub fn new(epoch_length: u64, min_validators: usize) -> Self {
        PoSConsensus {
            staking_vault: StakingVault::default_vault(),
            validators: Vec::new(),
            current_proposer: None,
            epoch_length,
            min_validators,
        }
    }

    // ------------------------------------------------------------------
    // Validator management
    // ------------------------------------------------------------------

    /// Register a validator and lock their initial stake.
    ///
    /// Fails if:
    /// - `address` is already registered (`DuplicateValidator`)
    /// - `stake` is below the protocol minimum (`StakingError`)
    pub fn register_validator(
        &mut self,
        address: Address,
        stake: Amount,
        height: BlockHeight,
    ) -> Result<(), PoSError> {
        // Reject duplicates before touching the vault
        if self.validators.iter().any(|v| v.address == address) {
            return Err(PoSError::DuplicateValidator(address));
        }

        // Delegate staking to the vault; translate staking errors
        self.staking_vault
            .stake(address, stake, height)
            .map_err(|e| PoSError::StakingError(e.to_string()))?;

        self.validators.push(ValidatorInfo::new(address, stake));
        Ok(())
    }

    /// Remove a validator from the active set.
    ///
    /// Does **not** unstake — the caller should invoke the staking vault
    /// separately if they want to begin unbonding.
    pub fn remove_validator(&mut self, address: &Address) -> Result<(), PoSError> {
        let pos = self
            .validators
            .iter()
            .position(|v| &v.address == address)
            .ok_or(PoSError::ValidatorNotFound(*address))?;

        self.validators.swap_remove(pos);

        // Clear current proposer if it was the removed validator
        if self.current_proposer.as_ref() == Some(address) {
            self.current_proposer = None;
        }

        Ok(())
    }

    // ------------------------------------------------------------------
    // Proposer selection
    // ------------------------------------------------------------------

    /// Select the block proposer for `height` using weighted round-robin.
    ///
    /// The selection is deterministic: given the same validator set and height
    /// the result is always the same address.
    ///
    /// Algorithm:
    ///   1. Compute `slot = height % total_weight`
    ///   2. Walk validators in order, accumulating cumulative weight
    ///   3. First validator whose cumulative weight exceeds `slot` wins
    ///
    /// Fails with [`PoSError::InsufficientValidators`] if the active validator
    /// count is below `min_validators`.
    pub fn select_proposer(&mut self, height: BlockHeight) -> Result<Address, PoSError> {
        if self.validators.len() < self.min_validators {
            return Err(PoSError::InsufficientValidators(self.min_validators));
        }

        let total = self.total_voting_power();
        if total == 0 {
            return Err(PoSError::InsufficientValidators(self.min_validators));
        }

        let slot = height % total;
        let mut cumulative: u64 = 0;

        for v in &self.validators {
            cumulative += v.weight;
            if slot < cumulative {
                self.current_proposer = Some(v.address);
                return Ok(v.address);
            }
        }

        // Fallback: last validator (handles rounding edge-case)
        let addr = self.validators.last().unwrap().address;
        self.current_proposer = Some(addr);
        Ok(addr)
    }

    // ------------------------------------------------------------------
    // Block validation
    // ------------------------------------------------------------------

    /// Verify that `proposer` is the valid proposer for `height`.
    ///
    /// This re-runs proposer selection and compares; it does **not** mutate
    /// `current_proposer`.
    pub fn validate_block(
        &mut self,
        proposer: &Address,
        height: BlockHeight,
    ) -> Result<(), PoSError> {
        let expected = self.select_proposer(height)?;
        if &expected != proposer {
            return Err(PoSError::InvalidProposer(*proposer));
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Statistics
    // ------------------------------------------------------------------

    /// Increment `blocks_proposed` counter for `address`.
    pub fn record_block_proposed(&mut self, address: &Address) -> Result<(), PoSError> {
        let v = self
            .validators
            .iter_mut()
            .find(|v| &v.address == address)
            .ok_or(PoSError::ValidatorNotFound(*address))?;
        v.blocks_proposed += 1;
        Ok(())
    }

    /// Increment `blocks_missed` counter for `address`.
    pub fn record_block_missed(&mut self, address: &Address) -> Result<(), PoSError> {
        let v = self
            .validators
            .iter_mut()
            .find(|v| &v.address == address)
            .ok_or(PoSError::ValidatorNotFound(*address))?;
        v.blocks_missed += 1;
        Ok(())
    }

    // ------------------------------------------------------------------
    // Queries
    // ------------------------------------------------------------------

    /// Return the full validator slice.
    pub fn get_validators(&self) -> &[ValidatorInfo] {
        &self.validators
    }

    /// Look up a single validator by address.
    pub fn get_validator(&self, address: &Address) -> Option<&ValidatorInfo> {
        self.validators.iter().find(|v| &v.address == address)
    }

    /// Sum of all validator voting weights.
    pub fn total_voting_power(&self) -> u64 {
        self.validators.iter().map(|v| v.weight).sum()
    }

    /// Return `true` if `height` is the last block of an epoch.
    ///
    /// Height 0 (genesis) is **not** considered an epoch boundary.
    pub fn is_epoch_boundary(&self, height: BlockHeight) -> bool {
        height > 0 && height % self.epoch_length == 0
    }

    // ------------------------------------------------------------------
    // Slashing
    // ------------------------------------------------------------------

    /// Slash `percent_bps` basis points from `address`'s stake.
    ///
    /// Delegates to the underlying [`StakingVault`] and updates the cached
    /// stake and weight on the validator record.
    ///
    /// Returns the amount slashed.
    pub fn slash_validator(
        &mut self,
        address: &Address,
        percent_bps: u32,
    ) -> Result<Amount, PoSError> {
        let slashed = self
            .staking_vault
            .slash(address, percent_bps)
            .map_err(|e| PoSError::StakingError(e.to_string()))?;

        // Keep cached stake/weight in sync with the vault
        if let Some(v) = self.validators.iter_mut().find(|v| &v.address == address) {
            v.stake = v.stake.saturating_sub(slashed);
            v.weight = stake_to_weight(v.stake);
        }

        Ok(slashed)
    }
}
