use crate::compute_market::*;
use crate::types::*;

#[test]
fn test_capacity_deduction_on_accept() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    let initial_capacity = ComputeCapacity::new(4000, 8192, 100);
    let job_capacity = ComputeCapacity::new(1000, 2048, 10);

    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            initial_capacity.clone(),
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
            job_capacity.clone(),
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        )
        .unwrap();

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(
        provider.available_capacity.cpu_millicores,
        initial_capacity.cpu_millicores - job_capacity.cpu_millicores
    );
    assert_eq!(
        provider.available_capacity.memory_mb,
        initial_capacity.memory_mb - job_capacity.memory_mb
    );
}

#[test]
fn test_capacity_restoration_on_cancel() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);
    let job_id = Hash::hash_data(b"test-job");

    let initial_capacity = ComputeCapacity::new(4000, 8192, 100);
    let job_capacity = ComputeCapacity::new(1000, 2048, 10);

    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            initial_capacity.clone(),
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
            job_capacity,
            1_000_000_000_000_000_000,
            3600,
            2_000_000_000_000_000_000,
            1000000,
        )
        .unwrap();

    market
        .accept_job(job_id, provider_addr, 1_000_000_000_000_000_000)
        .unwrap();

    market.cancel_job(job_id, user_addr, 1000001).unwrap();

    let provider = market.get_provider(&provider_addr).unwrap();
    assert_eq!(
        provider.available_capacity.cpu_millicores,
        initial_capacity.cpu_millicores
    );
    assert_eq!(
        provider.available_capacity.memory_mb,
        initial_capacity.memory_mb
    );
}

#[test]
fn test_insufficient_capacity_rejection() {
    let mut market = ComputeMarketState::new();
    let provider_addr = Address::from([1u8; 20]);
    let user_addr = Address::from([2u8; 20]);

    market
        .register_provider(
            provider_addr,
            vec![ResourceType::VM],
            ComputeCapacity::new(1000, 2048, 10),
            1_000_000_000_000_000_000,
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider.example.com".to_string(),
        )
        .unwrap();

    let result = market.create_job(
        Hash::hash_data(b"test-job"),
        user_addr,
        Some(provider_addr),
        ResourceType::VM,
        ComputeCapacity::new(8000, 16384, 100),
        1_000_000_000_000_000_000,
        3600,
        2_000_000_000_000_000_000,
        1000000,
    );

    assert!(matches!(
        result,
        Err(ComputeMarketError::InsufficientCapacity)
    ));
}

#[test]
fn test_find_matching_providers() {
    let mut market = ComputeMarketState::new();

    let provider1 = Address::from([1u8; 20]);
    let provider2 = Address::from([2u8; 20]);
    let provider3 = Address::from([3u8; 20]);

    market
        .register_provider(
            provider1,
            vec![ResourceType::VM],
            ComputeCapacity::new(4000, 8192, 100),
            500_000_000_000_000_000, // 0.5 ISA
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider1.example.com".to_string(),
        )
        .unwrap();

    market
        .register_provider(
            provider2,
            vec![ResourceType::VM, ResourceType::Browser],
            ComputeCapacity::new(8000, 16384, 200),
            2_000_000_000_000_000_000, // 2 ISA
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider2.example.com".to_string(),
        )
        .unwrap();

    market
        .register_provider(
            provider3,
            vec![ResourceType::Browser],
            ComputeCapacity::new(2000, 4096, 50),
            300_000_000_000_000_000, // 0.3 ISA
            3600,
            86400,
            constants::PROVIDER_MIN_STAKE,
            None,
            "https://provider3.example.com".to_string(),
        )
        .unwrap();

    let matches = market.find_matching_providers(
        &ResourceType::VM,
        &ComputeCapacity::new(2000, 4096, 50),
        1_000_000_000_000_000_000,
    );

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].address, provider1);

    let matches = market.find_matching_providers(
        &ResourceType::VM,
        &ComputeCapacity::new(2000, 4096, 50),
        3_000_000_000_000_000_000,
    );

    assert_eq!(matches.len(), 2);
}

#[test]
fn test_list_user_jobs() {
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

    for i in 0..3 {
        market
            .create_job(
                Hash::hash_data(format!("job-{}", i).as_bytes()),
                user_addr,
                None,
                ResourceType::VM,
                ComputeCapacity::new(1000, 2048, 10),
                1_000_000_000_000_000_000,
                3600,
                2_000_000_000_000_000_000,
                1000000 + i as u64,
            )
            .unwrap();
    }

    let user_jobs = market.list_user_jobs(&user_addr);
    assert_eq!(user_jobs.len(), 3);
}

#[test]
fn test_list_providers_by_resource() {
    let mut market = ComputeMarketState::new();

    for i in 0..5 {
        let addr = Address::from([i as u8; 20]);
        let resources = if i % 2 == 0 {
            vec![ResourceType::VM]
        } else {
            vec![ResourceType::Browser]
        };

        market
            .register_provider(
                addr,
                resources,
                ComputeCapacity::new(4000, 8192, 100),
                1_000_000_000_000_000_000,
                3600,
                86400,
                constants::PROVIDER_MIN_STAKE,
                None,
                format!("https://provider{}.example.com", i),
            )
            .unwrap();
    }

    let vm_providers = market.list_providers_by_resource(&ResourceType::VM);
    assert_eq!(vm_providers.len(), 3);

    let browser_providers = market.list_providers_by_resource(&ResourceType::Browser);
    assert_eq!(browser_providers.len(), 2);
}
