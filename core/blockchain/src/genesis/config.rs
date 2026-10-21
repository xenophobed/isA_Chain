use super::types::*;
use super::vesting::VestingSchedule;
use crate::types::constants::{
    GENESIS_TIMESTAMP, INITIAL_SUPPLY, MAIN_CHAIN_ID, TEST_CHAIN_ID, VALIDATOR_MIN_STAKE,
};
use crate::types::{Address, Amount, BlockHeight, ChainId, Timestamp};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

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
        let ecosystem_amount = one_billion * 40 / 100; // 400 M
        let team_amount = one_billion * 20 / 100; // 200 M
        let treasury_amount = one_billion * 15 / 100; // 150 M
        let provider_incentives_amount = one_billion * 15 / 100; // 150 M
        let early_supporter_amount = one_billion * 10 / 100; // 100 M

        let admin = Address::from([0xAD; 20]);

        let ecosystem_addr = Address::from([0x01; 20]);
        let team_addr = Address::from([0x02; 20]);
        let treasury_addr = Address::from([0x03; 20]);
        let provider_incentives_addr = Address::from([0x04; 20]);
        let early_supporter_addr = Address::from([0x05; 20]);

        // Immediate allocations (non-vested).
        let allocations = vec![GenesisAllocation {
            address: treasury_addr,
            amount: treasury_amount,
            purpose: AllocationPurpose::Treasury,
        }];

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
        let immediate: Amount = self.allocations.iter().map(|a| a.amount).sum();

        let vested: Amount = self
            .vesting_schedules
            .iter()
            .map(|v| v.total_amount)
            .sum();

        immediate.saturating_add(vested)
    }
}
