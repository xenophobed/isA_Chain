use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_job_lifecycle() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job-1");

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

    assert_eq!(market.stats.total_jobs_created, 1);

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();

    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Matched);

    market.start_job(job_id, provider_addr, 1000001).unwrap();

    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Running);

    let result = market
        .settle_job(
            job_id,
            ComputeUsage {
                duration_secs: 3600,
                cpu_seconds: 3600,
                memory_mb_seconds: 2048 * 3600,
                network_bytes: 1_000_000,
                storage_bytes: 10_000_000_000,
            },
            2000000,
        )
        .unwrap();

    assert!(result.provider_payment > 0);
    assert_eq!(market.stats.total_jobs_completed, 1);
}

#[test]
fn test_accept_job_wrong_provider() {
    let mut market = ComputeMarketState::new();
    let provider1 = Address::from([1u8; 20]);
    let provider2 = Address::from([2u8; 20]);
    let user_addr = Address::from([3u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    for (addr, idx) in [(provider1, 1), (provider2, 2)] {
        market
            .register_provider(
                addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                format!("https://provider{}.example.com", idx),
            )
            .unwrap();
    }

    market
        .create_job(
            job_id,
            user_addr,
            Some(provider1),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        )
        .unwrap();

    let result = market.accept_job(job_id, provider2, 1_000_000_000_000_000_000);
    assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
}

#[test]
fn test_accept_job_price_too_high() {
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

    let result = market.accept_job(job_id, provider_addr, 2_000_000_000_000_000_000);
    assert!(matches!(result, Err(ComputeMarketError::PriceExceedsMax)));
}

#[test]
fn test_accept_job_already_accepted() {
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

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();

    let result = market.accept_job(job_id, provider_addr, 1_000_000_000_000_000_000);
    assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
}

#[test]
fn test_start_job_wrong_provider() {
    let mut market = ComputeMarketState::new();
    let provider1 = Address::from([1u8; 20]);
    let provider2 = Address::from([2u8; 20]);
    let user_addr = Address::from([3u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    for (addr, idx) in [(provider1, 1), (provider2, 2)] {
        market
            .register_provider(
                addr,
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                format!("https://provider{}.example.com", idx),
            )
            .unwrap();
    }

    market
        .create_job(
            job_id,
            user_addr,
            Some(provider1),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        )
        .unwrap();

    market
        .accept_job(job_id, provider1, 1_000_000_000_000_000_000)
        .unwrap();

    let result = market.start_job(job_id, provider2, 1000001);
    assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
}

#[test]
fn test_start_job_not_matched() {
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

    let result = market.start_job(job_id, provider_addr, 1000001);
    assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
}
