use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_settle_job_not_running() {
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

    let result = market.settle_job(
        job_id,
        ComputeUsage {
            duration_secs: 3600,
            cpu_seconds: 3600,
            memory_mb_seconds: 2048 * 3600,
            network_bytes: 1_000_000,
            storage_bytes: 10_000_000_000,
        },
        2000000,
    );

    assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
}

#[test]
fn test_settle_job_protocol_fee() {
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
    market.start_job(job_id, provider_addr, 1000001).unwrap();

    let initial_treasury = market.treasury_balance;

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

    assert!(result.protocol_fee > 0);
    assert_eq!(
        market.treasury_balance,
        initial_treasury + result.protocol_fee
    );

    let total_payment = result.provider_payment + result.protocol_fee;
    let expected_fee = (total_payment * constants::PROTOCOL_FEE_PERCENT as u128) / 10000;
    assert_eq!(result.protocol_fee, expected_fee);
}

#[test]
fn test_cancel_job_by_user() {
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

    let escrow = 2_000_000_000_000_000_000u128;
    market
        .create_job(
            job_id,
            user_addr,
            Some(provider_addr),
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            escrow,
            1000000,
        )
        .unwrap();

    let refund = market.cancel_job(job_id, user_addr, 1000001).unwrap();
    assert_eq!(refund, escrow);

    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Cancelled);
}

#[test]
fn test_cancel_job_by_provider() {
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

    let result = market.cancel_job(job_id, provider_addr, 1000001);
    assert!(result.is_ok());
}

#[test]
fn test_cancel_job_unauthorized() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let random_addr = Address::from([99u8; 20]);
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

    let result = market.cancel_job(job_id, random_addr, 1000001);
    assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
}

#[test]
fn test_cancel_running_job_fails() {
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
    market.start_job(job_id, provider_addr, 1000001).unwrap();

    let result = market.cancel_job(job_id, user_addr, 1000002);
    assert!(matches!(result, Err(ComputeMarketError::InvalidJobState)));
}
