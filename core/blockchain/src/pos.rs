//! Proof-of-Stake consensus engine for isA Chain.
//!
//! This module implements a weighted round-robin PoS consensus where validators
//! are selected as block proposers proportionally to their staked amount.

use crate::staking::StakingVault;
use crate::types::{Address, Amount, BlockHeight};
use crate::types::constants::VALIDATOR_MIN_STAKE;

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur in the PoS consensus engine.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PoSError {
    #[error("Insufficient validators: need at least {0}")]
    InsufficientValidators(usize),

    #[error("Invalid proposer: {0}")]
    InvalidProposer(Address),

    #[error("Insufficient voting power: required {required}, actual {actual}")]
    InsufficientVotingPower { required: u64, actual: u64 },

    #[error("Validator not found: {0}")]
    ValidatorNotFound(Address),

    #[error("Duplicate validator: {0}")]
    DuplicateValidator(Address),

    #[error("Staking error: {0}")]
    StakingError(String),
}

// ============================================================================
// ValidatorInfo
// ============================================================================

/// Metadata tracked per active validator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorInfo {
    /// On-chain address of the validator
    pub address: Address,
    /// Active stake amount (in base ISA units)
    pub stake: Amount,
    /// Voting weight — proportional to stake, scaled to u64
    pub weight: u64,
    /// Total blocks this validator has successfully proposed
    pub blocks_proposed: u64,
    /// Total blocks this validator was selected but missed
    pub blocks_missed: u64,
}

impl ValidatorInfo {
    fn new(address: Address, stake: Amount) -> Self {
        ValidatorInfo {
            address,
            stake,
            weight: stake_to_weight(stake),
            blocks_proposed: 0,
            blocks_missed: 0,
        }
    }
}

/// Convert a raw stake amount to a u64 voting weight.
///
/// We divide by VALIDATOR_MIN_STAKE so that one minimum-stake unit = 1 weight.
/// This keeps the numbers small enough to avoid u64 overflow in
/// cumulative-weight arithmetic while preserving proportionality.
fn stake_to_weight(stake: Amount) -> u64 {
    // Guard against zero min-stake (shouldn't happen in production)
    if VALIDATOR_MIN_STAKE == 0 {
        return stake as u64;
    }
    // Safe: VALIDATOR_MIN_STAKE is 32_000 ISA in its base unit;
    // even with 1B tokens staked the result fits comfortably in u64.
    (stake / VALIDATOR_MIN_STAKE) as u64
}

// ============================================================================
// PoSConsensus
// ============================================================================

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::constants::VALIDATOR_MIN_STAKE;

    // ---- helpers -----------------------------------------------------------

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    /// Default engine: epoch=100, min_validators=1.
    fn engine() -> PoSConsensus {
        PoSConsensus::new(100, 1)
    }

    // -----------------------------------------------------------------------

    #[test]
    fn test_register_validator() {
        let mut e = engine();
        let a = addr(0x01);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let validators = e.get_validators();
        assert_eq!(validators.len(), 1);
        assert_eq!(validators[0].address, a);
        assert_eq!(validators[0].stake, VALIDATOR_MIN_STAKE);
        assert_eq!(validators[0].weight, 1);
        assert!(e.staking_vault.is_staked(&a));
    }

    #[test]
    fn test_register_duplicate_fails() {
        let mut e = engine();
        let a = addr(0x02);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
        let result = e.register_validator(a, VALIDATOR_MIN_STAKE, 2);
        assert_eq!(result, Err(PoSError::DuplicateValidator(a)));
        // Only one entry should exist
        assert_eq!(e.get_validators().len(), 1);
    }

    #[test]
    fn test_remove_validator() {
        let mut e = engine();
        let a = addr(0x03);
        let b = addr(0x04);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
        e.register_validator(b, VALIDATOR_MIN_STAKE, 1).unwrap();

        e.remove_validator(&a).unwrap();
        assert_eq!(e.get_validators().len(), 1);
        assert!(e.get_validator(&a).is_none());
        assert!(e.get_validator(&b).is_some());
    }

    #[test]
    fn test_remove_nonexistent_validator_fails() {
        let mut e = engine();
        let a = addr(0xAA);
        assert_eq!(
            e.remove_validator(&a),
            Err(PoSError::ValidatorNotFound(a))
        );
    }

    #[test]
    fn test_select_proposer_deterministic() {
        let mut e = engine();
        let a = addr(0x05);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        // Same height always returns the same address
        let p1 = e.select_proposer(42).unwrap();
        let p2 = e.select_proposer(42).unwrap();
        assert_eq!(p1, p2);
    }

    #[test]
    fn test_select_proposer_weighted() {
        let mut e = engine();
        // Validator A: weight 1 (1× min stake)
        // Validator B: weight 3 (3× min stake)
        // Total weight = 4; A wins for slot 0, B wins for slots 1, 2, 3
        let a = addr(0x0A);
        let b = addr(0x0B);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
        e.register_validator(b, VALIDATOR_MIN_STAKE * 3, 1).unwrap();

        assert_eq!(e.total_voting_power(), 4);

        // Slot 0 → A (cumulative weight after A = 1 > 0)
        let proposer_0 = e.select_proposer(0).unwrap(); // slot = 0 % 4 = 0
        assert_eq!(proposer_0, a);

        // Slots 1, 2, 3 → B
        let proposer_1 = e.select_proposer(1).unwrap(); // slot = 1
        let proposer_2 = e.select_proposer(2).unwrap(); // slot = 2
        let proposer_3 = e.select_proposer(3).unwrap(); // slot = 3
        assert_eq!(proposer_1, b);
        assert_eq!(proposer_2, b);
        assert_eq!(proposer_3, b);
    }

    #[test]
    fn test_validate_block() {
        let mut e = engine();
        let a = addr(0x0C);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        // The single validator should always be the valid proposer
        assert!(e.validate_block(&a, 10).is_ok());
    }

    #[test]
    fn test_validate_wrong_proposer() {
        let mut e = engine();
        let a = addr(0x0D);
        let wrong = addr(0xFF);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let result = e.validate_block(&wrong, 10);
        assert_eq!(result, Err(PoSError::InvalidProposer(wrong)));
    }

    #[test]
    fn test_insufficient_validators() {
        // Engine that requires at least 2 validators
        let mut e = PoSConsensus::new(100, 2);
        let a = addr(0x0E);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        let result = e.select_proposer(1);
        assert_eq!(result, Err(PoSError::InsufficientValidators(2)));
    }

    #[test]
    fn test_record_proposed_and_missed() {
        let mut e = engine();
        let a = addr(0x0F);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        e.record_block_proposed(&a).unwrap();
        e.record_block_proposed(&a).unwrap();
        e.record_block_missed(&a).unwrap();

        let v = e.get_validator(&a).unwrap();
        assert_eq!(v.blocks_proposed, 2);
        assert_eq!(v.blocks_missed, 1);
    }

    #[test]
    fn test_slash_validator() {
        let mut e = engine();
        let a = addr(0x10);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();

        // Slash 10% (1000 bps)
        let slashed = e.slash_validator(&a, 1000).unwrap();
        let expected = VALIDATOR_MIN_STAKE / 10;
        assert_eq!(slashed, expected);

        let v = e.get_validator(&a).unwrap();
        assert_eq!(v.stake, VALIDATOR_MIN_STAKE - expected);
        // weight should be updated
        assert_eq!(v.weight, stake_to_weight(VALIDATOR_MIN_STAKE - expected));
    }

    #[test]
    fn test_epoch_boundary() {
        let e = PoSConsensus::new(100, 1);

        // Block 0 is NOT an epoch boundary
        assert!(!e.is_epoch_boundary(0));
        // Not multiples of epoch_length
        assert!(!e.is_epoch_boundary(1));
        assert!(!e.is_epoch_boundary(99));
        // Exact multiples are epoch boundaries
        assert!(e.is_epoch_boundary(100));
        assert!(e.is_epoch_boundary(200));
        assert!(e.is_epoch_boundary(500));
    }

    #[test]
    fn test_total_voting_power() {
        let mut e = engine();
        assert_eq!(e.total_voting_power(), 0);

        let a = addr(0x20);
        let b = addr(0x21);
        e.register_validator(a, VALIDATOR_MIN_STAKE, 1).unwrap();
        assert_eq!(e.total_voting_power(), 1);

        e.register_validator(b, VALIDATOR_MIN_STAKE * 4, 1).unwrap();
        assert_eq!(e.total_voting_power(), 5);

        e.remove_validator(&a).unwrap();
        assert_eq!(e.total_voting_power(), 4);
    }
}
