use crate::types::{Address, Amount, BlockHeight, ChainId, Timestamp};
use crate::types::constants::{
    INITIAL_SUPPLY, MAIN_CHAIN_ID, TEST_CHAIN_ID, GENESIS_TIMESTAMP, VALIDATOR_MIN_STAKE,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// ============================================================================
// Allocation Purpose
// ============================================================================

/// Categories for genesis token allocations.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AllocationPurpose {
    Team,
    Treasury,
    Ecosystem,
    Validators,
    ProviderIncentives,
    EarlySupporter,
}

// ============================================================================
// Genesis Allocation
// ============================================================================

/// A single recipient entry in the genesis distribution.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisAllocation {
    pub address: Address,
    pub amount: Amount,
    pub purpose: AllocationPurpose,
}

// ============================================================================
// Vesting Schedule
// ============================================================================

/// Linear vesting schedule with an optional cliff.
///
/// No tokens vest before `cliff_height`.  Between `cliff_height` and
/// `end_height` vesting is linear (proportional to elapsed blocks).
/// At and after `end_height` the full `total_amount` is vested.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VestingSchedule {
    pub beneficiary: Address,
    pub total_amount: Amount,
    /// Tokens already released to the beneficiary.
    pub released: Amount,
    pub start_height: BlockHeight,
    /// No tokens vest before this height.
    pub cliff_height: BlockHeight,
    /// Fully vested at (and after) this height.
    pub end_height: BlockHeight,
    pub purpose: AllocationPurpose,
}

impl VestingSchedule {
    /// How much has vested at `current_height` (regardless of what has
    /// already been released).
    pub fn vested_amount(&self, current_height: BlockHeight) -> Amount {
        if current_height < self.cliff_height {
            return 0;
        }

        if current_height >= self.end_height {
            return self.total_amount;
        }

        // Linear vesting between start_height and end_height.
        // Use the later of start_height / cliff_height as the vesting
        // start so that the cliff jump is immediate.
        let vesting_start = self.start_height.max(self.cliff_height);

        if self.end_height <= vesting_start {
            // Degenerate schedule — treat as fully vested at cliff.
            return self.total_amount;
        }

        let elapsed = current_height.saturating_sub(vesting_start) as u128;
        let total_duration = (self.end_height - vesting_start) as u128;

        // Use u128 arithmetic to avoid overflow on large amounts.
        self.total_amount
            .saturating_mul(elapsed)
            / total_duration
    }

    /// Tokens that have vested but have not yet been released.
    pub fn releasable(&self, current_height: BlockHeight) -> Amount {
        self.vested_amount(current_height)
            .saturating_sub(self.released)
    }

    /// Release all currently releasable tokens.  Updates `self.released`
    /// and returns the released amount.
    pub fn release(&mut self, current_height: BlockHeight) -> Amount {
        let amount = self.releasable(current_height);
        self.released = self.released.saturating_add(amount);
        amount
    }

    /// Returns `true` once all tokens have vested.
    pub fn is_fully_vested(&self, current_height: BlockHeight) -> bool {
        current_height >= self.end_height
    }
}

// ============================================================================
// Genesis Error
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum GenesisError {
    #[error("Total allocations exceed initial supply")]
    AllocationExceedsSupply,

    #[error("Duplicate allocation for address {0}")]
    DuplicateAllocation(Address),

    #[error("Invalid vesting schedule: cliff > end or amount = 0")]
    InvalidVestingSchedule,

    #[error("Initial supply must be greater than zero")]
    ZeroSupply,
}

// ============================================================================
// Genesis Config
// ============================================================================

/// Full genesis configuration for the chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GenesisConfig {
    pub chain_id: ChainId,
    pub timestamp: Timestamp,
    pub initial_supply: Amount,
    /// Immediate (non-vested) allocations.
    pub allocations: Vec<GenesisAllocation>,
    /// Time-locked allocations that vest linearly.
    pub vesting_schedules: Vec<VestingSchedule>,
    /// Protocol admin address.
    pub admin: Address,
    /// Treasury fee in basis points (e.g. 250 = 2.5%).
    pub treasury_fee_rate_bps: u32,
    /// Minimum stake required to become a validator.
    pub validator_min_stake: Amount,
    /// Initial ISA/USD oracle price in micro-USD (e.g. 1_000_000 = $1.00).
    pub oracle_initial_price: Amount,
}

impl GenesisConfig {
    // -----------------------------------------------------------------------
    // Constructors
    // -----------------------------------------------------------------------

    /// Default mainnet genesis with the canonical ISA token economics.
    ///
    /// Token distribution (1 B ISA total):
    /// - Community & Ecosystem  40% — 400 M ISA (4-year vest, 6-month cliff)
    /// - Team & Contributors    20% — 200 M ISA (4-year vest, 1-year cliff)
    /// - Treasury               15% — 150 M ISA (immediate)
    /// - Provider Incentives    15% — 150 M ISA (10-year vest, no cliff)
    /// - Early Supporters       10% — 100 M ISA (2-year vest, 6-month cliff)
    pub fn default_mainnet() -> Self {
        // Approximate block heights (3-second blocks):
        //   6 months  ≈  5_256_000 blocks
        //   1 year    ≈ 10_512_000 blocks
        //   2 years   ≈ 21_024_000 blocks
        //   4 years   ≈ 42_048_000 blocks
        //  10 years   ≈ 105_120_000 blocks
        const BLOCKS_PER_YEAR: BlockHeight = 10_512_000;
        const CLIFF_6_MONTHS: BlockHeight = BLOCKS_PER_YEAR / 2;
        const CLIFF_1_YEAR: BlockHeight = BLOCKS_PER_YEAR;
        const END_2_YEARS: BlockHeight = BLOCKS_PER_YEAR * 2;
        const END_4_YEARS: BlockHeight = BLOCKS_PER_YEAR * 4;
        const END_10_YEARS: BlockHeight = BLOCKS_PER_YEAR * 10;

        let one_billion: Amount = INITIAL_SUPPLY; // 1B ISA in wei

        // Percentages expressed as numerator / 100 of total supply.
        let ecosystem_amount = one_billion * 40 / 100;          // 400 M
        let team_amount = one_billion * 20 / 100;               // 200 M
        let treasury_amount = one_billion * 15 / 100;           // 150 M
        let provider_incentives_amount = one_billion * 15 / 100;// 150 M
        let early_supporter_amount = one_billion * 10 / 100;    // 100 M

        let admin = Address::from([0xAD; 20]);

        let ecosystem_addr = Address::from([0x01; 20]);
        let team_addr = Address::from([0x02; 20]);
        let treasury_addr = Address::from([0x03; 20]);
        let provider_incentives_addr = Address::from([0x04; 20]);
        let early_supporter_addr = Address::from([0x05; 20]);

        // Immediate allocations (non-vested).
        let allocations = vec![
            GenesisAllocation {
                address: treasury_addr,
                amount: treasury_amount,
                purpose: AllocationPurpose::Treasury,
            },
        ];

        // Vested allocations.
        let vesting_schedules = vec![
            VestingSchedule {
                beneficiary: ecosystem_addr,
                total_amount: ecosystem_amount,
                released: 0,
                start_height: 0,
                cliff_height: CLIFF_6_MONTHS,
                end_height: END_4_YEARS,
                purpose: AllocationPurpose::Ecosystem,
            },
            VestingSchedule {
                beneficiary: team_addr,
                total_amount: team_amount,
                released: 0,
                start_height: 0,
                cliff_height: CLIFF_1_YEAR,
                end_height: END_4_YEARS,
                purpose: AllocationPurpose::Team,
            },
            VestingSchedule {
                beneficiary: provider_incentives_addr,
                total_amount: provider_incentives_amount,
                released: 0,
                start_height: 0,
                cliff_height: 0,
                end_height: END_10_YEARS,
                purpose: AllocationPurpose::ProviderIncentives,
            },
            VestingSchedule {
                beneficiary: early_supporter_addr,
                total_amount: early_supporter_amount,
                released: 0,
                start_height: 0,
                cliff_height: CLIFF_6_MONTHS,
                end_height: END_2_YEARS,
                purpose: AllocationPurpose::EarlySupporter,
            },
        ];

        GenesisConfig {
            chain_id: MAIN_CHAIN_ID,
            timestamp: GENESIS_TIMESTAMP,
            initial_supply: INITIAL_SUPPLY,
            allocations,
            vesting_schedules,
            admin,
            treasury_fee_rate_bps: 250, // 2.5%
            validator_min_stake: VALIDATOR_MIN_STAKE,
            oracle_initial_price: 1_000_000, // $1.00 in micro-USD
        }
    }

    /// Simplified testnet genesis.
    pub fn default_testnet() -> Self {
        let admin = Address::from([0xAD; 20]);
        let faucet = Address::from([0xFA; 20]);

        let allocations = vec![GenesisAllocation {
            address: faucet,
            amount: INITIAL_SUPPLY,
            purpose: AllocationPurpose::Treasury,
        }];

        GenesisConfig {
            chain_id: TEST_CHAIN_ID,
            timestamp: GENESIS_TIMESTAMP,
            initial_supply: INITIAL_SUPPLY,
            allocations,
            vesting_schedules: vec![],
            admin,
            treasury_fee_rate_bps: 250,
            validator_min_stake: VALIDATOR_MIN_STAKE,
            oracle_initial_price: 1_000_000,
        }
    }

    // -----------------------------------------------------------------------
    // Validation
    // -----------------------------------------------------------------------

    /// Validate genesis configuration.
    ///
    /// Checks:
    /// 1. Initial supply is non-zero.
    /// 2. No address appears twice across allocations + vesting schedules.
    /// 3. Sum of all allocated amounts ≤ initial supply.
    /// 4. Each vesting schedule has a non-zero amount and cliff ≤ end.
    pub fn validate(&self) -> Result<(), GenesisError> {
        if self.initial_supply == 0 {
            return Err(GenesisError::ZeroSupply);
        }

        // Check for duplicate addresses across allocations and vesting.
        let mut seen: HashSet<Address> = HashSet::new();

        for alloc in &self.allocations {
            if !seen.insert(alloc.address) {
                return Err(GenesisError::DuplicateAllocation(alloc.address));
            }
        }

        for vest in &self.vesting_schedules {
            if !seen.insert(vest.beneficiary) {
                return Err(GenesisError::DuplicateAllocation(vest.beneficiary));
            }
            if vest.total_amount == 0 || vest.cliff_height > vest.end_height {
                return Err(GenesisError::InvalidVestingSchedule);
            }
        }

        // Check total allocated ≤ initial supply.
        let total = self.total_allocated();
        if total > self.initial_supply {
            return Err(GenesisError::AllocationExceedsSupply);
        }

        Ok(())
    }

    /// Sum of all immediately allocated amounts plus all vesting amounts.
    pub fn total_allocated(&self) -> Amount {
        let immediate: Amount = self
            .allocations
            .iter()
            .map(|a| a.amount)
            .sum();

        let vested: Amount = self
            .vesting_schedules
            .iter()
            .map(|v| v.total_amount)
            .sum();

        immediate.saturating_add(vested)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // GenesisConfig tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_mainnet_valid() {
        let config = GenesisConfig::default_mainnet();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_default_testnet_valid() {
        let config = GenesisConfig::default_testnet();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_allocation_exceeds_supply() {
        let mut config = GenesisConfig::default_testnet();
        // Push a second allocation that tips over the supply.
        config.allocations.push(GenesisAllocation {
            address: Address::from([0xFF; 20]),
            amount: 1, // faucet already holds full supply
            purpose: AllocationPurpose::Treasury,
        });
        assert_eq!(config.validate(), Err(GenesisError::AllocationExceedsSupply));
    }

    #[test]
    fn test_duplicate_allocation() {
        let mut config = GenesisConfig::default_testnet();
        let dup_addr = config.allocations[0].address;
        config.allocations.push(GenesisAllocation {
            address: dup_addr,
            amount: 0, // Amount doesn't matter — duplicate check comes first.
            purpose: AllocationPurpose::Treasury,
        });
        assert_eq!(
            config.validate(),
            Err(GenesisError::DuplicateAllocation(dup_addr))
        );
    }

    #[test]
    fn test_invalid_vesting_schedule() {
        let mut config = GenesisConfig::default_testnet();
        // cliff_height > end_height → invalid.
        config.vesting_schedules.push(VestingSchedule {
            beneficiary: Address::from([0xBE; 20]),
            total_amount: 1_000,
            released: 0,
            start_height: 0,
            cliff_height: 1000,
            end_height: 500, // cliff > end
            purpose: AllocationPurpose::Team,
        });
        assert_eq!(
            config.validate(),
            Err(GenesisError::InvalidVestingSchedule)
        );
    }

    #[test]
    fn test_zero_supply_rejected() {
        let mut config = GenesisConfig::default_testnet();
        config.initial_supply = 0;
        config.allocations.clear();
        config.vesting_schedules.clear();
        assert_eq!(config.validate(), Err(GenesisError::ZeroSupply));
    }

    #[test]
    fn test_total_allocated() {
        let config = GenesisConfig::default_mainnet();
        let expected = INITIAL_SUPPLY;
        assert_eq!(config.total_allocated(), expected);
    }

    #[test]
    fn test_mainnet_allocations_sum() {
        // All allocations + vesting schedules must sum to exactly 1B ISA.
        let config = GenesisConfig::default_mainnet();
        assert_eq!(config.total_allocated(), INITIAL_SUPPLY);
    }

    // -----------------------------------------------------------------------
    // VestingSchedule tests
    // -----------------------------------------------------------------------

    fn make_vest(
        total_amount: Amount,
        start_height: BlockHeight,
        cliff_height: BlockHeight,
        end_height: BlockHeight,
    ) -> VestingSchedule {
        VestingSchedule {
            beneficiary: Address::from([0xAA; 20]),
            total_amount,
            released: 0,
            start_height,
            cliff_height,
            end_height,
            purpose: AllocationPurpose::Team,
        }
    }

    #[test]
    fn test_vesting_before_cliff() {
        let vest = make_vest(1_000, 0, 100, 1000);
        // Before the cliff nothing vests.
        assert_eq!(vest.vested_amount(0), 0);
        assert_eq!(vest.vested_amount(50), 0);
        assert_eq!(vest.vested_amount(99), 0);
    }

    #[test]
    fn test_vesting_after_cliff_linear() {
        // Vesting: start=0, cliff=0, end=1000, total=1000
        // After 500 blocks exactly 500 tokens should be vested.
        let vest = make_vest(1_000, 0, 0, 1000);
        assert_eq!(vest.vested_amount(500), 500);
        assert_eq!(vest.vested_amount(250), 250);
        assert_eq!(vest.vested_amount(750), 750);
    }

    #[test]
    fn test_vesting_fully_vested() {
        let vest = make_vest(1_000, 0, 100, 1000);
        assert_eq!(vest.vested_amount(1000), 1_000);
        assert_eq!(vest.vested_amount(2000), 1_000);
        assert!(vest.is_fully_vested(1000));
        assert!(!vest.is_fully_vested(999));
    }

    #[test]
    fn test_vesting_release() {
        let mut vest = make_vest(1_000, 0, 0, 1000);

        // At height 500, 500 tokens are vested — release them.
        let released = vest.release(500);
        assert_eq!(released, 500);
        assert_eq!(vest.released, 500);

        // Releasing again at the same height yields 0 (already released).
        let second = vest.release(500);
        assert_eq!(second, 0);

        // At height 1000 the remaining 500 should become available.
        let final_release = vest.release(1000);
        assert_eq!(final_release, 500);
        assert_eq!(vest.released, 1_000);
    }

    #[test]
    fn test_releasable() {
        let mut vest = make_vest(1_000, 0, 0, 1000);

        // Nothing released yet — all vested tokens are releasable.
        assert_eq!(vest.releasable(500), 500);

        vest.release(500);
        // After releasing, releasable at same height = 0.
        assert_eq!(vest.releasable(500), 0);
        // At height 1000 the remaining 500 are releasable.
        assert_eq!(vest.releasable(1000), 500);
    }
}
