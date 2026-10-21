use super::*;
use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_escrow_tracking() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);

    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            ComputeCapacity::new(8000, 16384, 200),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider.example.com".to_string(),
        )
        .unwrap();

    assert_eq!(market.total_escrow, 0);

    let escrow1 = 2_000_000_000_000_000_000u128;
    let escrow2 = 3_000_000_000_000_000_000u128;

    market
        .create_job(
            Hash::hash_data(b"job-1"),
            user_addr,
            None,
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            escrow1,
            1000000,
        )
        .unwrap();

    assert_eq!(market.total_escrow, escrow1);

    market
        .create_job(
            Hash::hash_data(b"job-2"),
            user_addr,
            None,
            ResourceType::VM,
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            escrow2,
            1000001,
        )
        .unwrap();

    assert_eq!(market.total_escrow, escrow1 + escrow2);

    market
        .cancel_job(Hash::hash_data(b"job-1"), user_addr, 1000002)
        .unwrap();

    assert_eq!(market.total_escrow, escrow2);
}

#[test]
fn test_provider_reputation_update() {
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

    let initial_reputation = market.get_provider(&provider_addr).unwrap().reputation;

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

    let final_reputation = market.get_provider(&provider_addr).unwrap().reputation;
    assert!(final_reputation > initial_reputation);
}

#[test]
fn test_market_stats_accuracy() {
    let mut market = ComputeMarketState::new();

    for i in 0..3 {
        market
            .register_provider(
                Address::from([i as u8; 20]),
                vec![ResourceType::VM],
                create_test_capacity(),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                format!("https://provider{}.example.com", i),
            )
            .unwrap();
    }

    assert_eq!(market.stats.total_providers, 3);
    assert_eq!(market.stats.active_providers, 3);

    market
        .update_provider(
            Address::from([0u8; 20]),
            None,
            None,
            Some(ProviderStatus::Paused),
            None,
            None,
        )
        .unwrap();

    assert_eq!(market.stats.total_providers, 3);
    assert_eq!(market.stats.active_providers, 2);

    market.provider_exit(Address::from([1u8; 20])).unwrap();

    assert_eq!(market.stats.total_providers, 3);
    assert_eq!(market.stats.active_providers, 1);
}
