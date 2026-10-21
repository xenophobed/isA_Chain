use crate::genesis::config::GenesisConfig;
use crate::genesis::types::{AllocationPurpose, GenesisAllocation, GenesisError};
use crate::genesis::vesting::VestingSchedule;
use crate::types::constants::INITIAL_SUPPLY;
use crate::types::Address;

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
