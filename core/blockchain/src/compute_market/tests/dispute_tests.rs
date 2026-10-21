use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_open_dispute_on_running_job() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");
    let dispute_id = Hash::hash_data(b"dispute-1");

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

    let result = market.open_dispute(
        dispute_id,
        job_id,
        user_addr,
        DisputeType::UnderCapacity,
        Hash::hash_data(b"evidence"),
        1000002,
    );

    assert!(result.is_ok());
    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Disputed);
    assert_eq!(market.stats.total_jobs_disputed, 1);
}

#[test]
fn test_open_dispute_unauthorized() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let random_addr = Address::from([99u8; 20]);
    let job_id = Hash::hash_data(b"test-job");
    let dispute_id = Hash::hash_data(b"dispute-1");

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

    let result = market.open_dispute(
        dispute_id,
        job_id,
        random_addr,
        DisputeType::UnderCapacity,
        Hash::hash_data(b"evidence"),
        1000002,
    );

    assert!(matches!(result, Err(ComputeMarketError::NotAuthorized)));
}

#[test]
fn test_resolve_dispute_with_slashing() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let resolver_addr = Address::from([99u8; 20]);
    let job_id = Hash::hash_data(b"test-job");
    let dispute_id = Hash::hash_data(b"dispute-1");

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

    let initial_stake = market.get_provider(&provider_addr).unwrap().stake;
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

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();
    market.start_job(job_id, provider_addr, 1000001).unwrap();

    market
        .open_dispute(
            dispute_id,
            job_id,
            user_addr,
            DisputeType::NonDelivery,
            Hash::hash_data(b"evidence"),
            1000002,
        )
        .unwrap();

    let slash_amount = 100_000_000_000_000_000_000u128; // 100 ISA
    let resolution = market
        .resolve_dispute(
            dispute_id,
            user_addr,          // User wins
            escrow,             // Full refund
            0,                  // No payment to provider
            slash_amount,       // Slash provider
            resolver_addr,
            1000003,
        )
        .unwrap();

    assert_eq!(resolution.winner, user_addr);
    assert_eq!(resolution.user_refund, escrow);
    assert_eq!(resolution.slash_amount, slash_amount);

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(provider.stake, initial_stake - slash_amount);
    assert_eq!(provider.jobs_failed, 1);

    let job = market.get_job(&job_id).unwrap();
    assert_eq!(job.status, JobStatus::Failed);
}

#[test]
fn test_resolve_dispute_already_resolved() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let resolver_addr = Address::from([99u8; 20]);
    let job_id = Hash::hash_data(b"test-job");
    let dispute_id = Hash::hash_data(b"dispute-1");

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

    market
        .open_dispute(
            dispute_id,
            job_id,
            user_addr,
            DisputeType::NonDelivery,
            Hash::hash_data(b"evidence"),
            1000002,
        )
        .unwrap();

    market
        .resolve_dispute(
            dispute_id,
            user_addr,
            2_000_000_000_000_000_000,
            0,
            0,
            resolver_addr,
            1000003,
        )
        .unwrap();

    let result = market.resolve_dispute(
        dispute_id,
        provider_addr,
        0,
        2_000_000_000_000_000_000,
        0,
        resolver_addr,
        1000004,
    );

    assert!(matches!(
        result,
        Err(ComputeMarketError::DisputeAlreadyResolved)
    ));
}
