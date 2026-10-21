use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_provider_registration() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);

    let result = market.register_provider(
        provider_addr,
        vec![ResourceType::VM, ResourceType::Browser],
        create_test_capacity(),
        1_000_000_000_000_000_000, // 1 ISA per hour
        3600,                       // 1 hour min
        86400,                      // 24 hours max
        constants::PROVIDER_MIN_STAKE,
        Some("us-west".to_string()),
        "https://provider.example.com".to_string(),
    );

    assert!(result.is_ok());
    assert_eq!(market.stats.total_providers, 1);
    assert_eq!(market.stats.active_providers, 1);

    // Should fail if registering same provider again
    let result2 = market.register_provider(
        provider_addr,
        vec![ResourceType::VM],
        create_test_capacity(),
        1_000_000_000_000_000_000,
        3600,
        86400,
        constants::PROVIDER_MIN_STAKE,
        None,
        "https://provider2.example.com".to_string(),
    );
    assert!(result2.is_err());
}

#[test]
fn test_provider_registration_insufficient_stake() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);

    let result = market.register_provider(
        provider_addr,
        vec![ResourceType::VM],
        create_test_capacity(),
        1_000_000_000_000_000_000,
        3600,
        86400,
        constants::PROVIDER_MIN_STAKE - 1, // Insufficient stake
        None,
        "https://provider.example.com".to_string(),
    );

    assert!(matches!(result, Err(ComputeMarketError::InsufficientStake)));
}

#[test]
fn test_provider_update() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);

    // Register provider
    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            create_test_capacity(),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider.example.com".to_string(),
        )
        .unwrap();

    // Update price
    let new_price = 2_000_000_000_000_000_000u128;
    market
        .update_provider(
            provider_addr,
            None,
            Some(new_price),
            None,
            None,
            None,
        )
        .unwrap();

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(provider.price_per_hour, new_price);

    // Update status to paused
    market
        .update_provider(
            provider_addr,
            None,
            None,
            Some(ProviderStatus::Paused),
            None,
            None,
        )
        .unwrap();

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(provider.status, ProviderStatus::Paused);
    assert_eq!(market.stats.active_providers, 0);

    // Update status back to active
    market
        .update_provider(
            provider_addr,
            None,
            None,
            Some(ProviderStatus::Active),
            None,
            None,
        )
        .unwrap();

    assert_eq!(market.stats.active_providers, 1);
}

#[test]
fn test_provider_update_nonexistent() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);

    let result = market.update_provider(
        provider_addr,
        None,
        Some(1_000_000_000_000_000_000),
        None,
        None,
        None,
    );

    assert!(matches!(result, Err(ComputeMarketError::ProviderNotFound)));
}

#[test]
fn test_provider_exit_success() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);

    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            create_test_capacity(),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider.example.com".to_string(),
        )
        .unwrap();

    let stake_returned = market.provider_exit(provider_addr).unwrap();
    assert_eq!(stake_returned, constants::PROVIDER_MIN_STAKE);

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(provider.status, ProviderStatus::Exited);
    assert_eq!(market.stats.active_providers, 0);
}

#[test]
fn test_provider_exit_with_active_job() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    // Register provider
    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            create_test_capacity(),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider.example.com".to_string(),
        )
        .unwrap();

    // Create and accept job
    market
        .create_job(
            job_id,
            user_addr,
            Some(provider_addr),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        )
        .unwrap();

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();

    // Try to exit - should fail
    let result = market.provider_exit(provider_addr);
    assert!(matches!(result, Err(ComputeMarketError::HasActiveJobs)));
}
