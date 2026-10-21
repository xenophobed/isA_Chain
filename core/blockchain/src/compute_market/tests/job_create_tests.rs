use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_create_job_nonexistent_provider() {
    let mut market = ComputeMarketState::new();
    let user_addr = Address::from([2u8; 20]);
    let nonexistent_provider = Address::from([99u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    let result = market.create_job(
        job_id,
        user_addr,
        Some(nonexistent_provider),
        ResourceType::VM,
        ComputeCapacity::new(1000, 2048, 10),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(matches!(result, Err(ComputeMarketError::ProviderNotFound)));
}

#[test]
fn test_create_job_inactive_provider() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

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

    let result = market.create_job(
        job_id,
        user_addr,
        Some(provider_addr),
        ResourceType::VM,
        ComputeCapacity::new(1000, 2048, 10),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(matches!(result, Err(ComputeMarketError::ProviderNotActive)));
}

#[test]
fn test_create_job_unsupported_resource_type() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

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

    let result = market.create_job(
        job_id,
        user_addr,
        Some(provider_addr),
        ResourceType::Browser, // Not supported
        ComputeCapacity::new(1000, 2048, 10),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(matches!(
        result,
        Err(ComputeMarketError::ResourceTypeNotSupported)
    ));
}

#[test]
fn test_create_job_duplicate_id() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

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

    let result = market.create_job(
        job_id,
        user_addr,
        Some(provider_addr),
        ResourceType::VM,
        ComputeCapacity::new(1000, 2048, 10),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(matches!(result, Err(ComputeMarketError::JobAlreadyExists)));
}

#[test]
fn test_create_job_without_specific_provider() {
    let mut market = ComputeMarketState::new();
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    let result = market.create_job(
        job_id,
        user_addr,
        None, // No specific provider
        ResourceType::VM,
        ComputeCapacity::new(1000, 2048, 10),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(result.is_ok());
    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Pending);
    assert!(job.provider.is_none());
}
