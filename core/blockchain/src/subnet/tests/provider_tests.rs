use super::*;
use crate::subnet::types::{
    isa, ProviderSubnetStatus, SubnetConfig, SubnetError, SubnetId, SubnetStatus,
};

#[test]
fn test_register_provider() {
    let mut r = initialized();
    let p = provider(0x01);
    let stake = isa(1_000); // exactly the Model minimum
    r.register_provider(SubnetId::Model, p, stake, 10).unwrap();

    let prov = r.get_provider(&SubnetId::Model, &p).unwrap();
    assert_eq!(prov.address, p);
    assert_eq!(prov.stake, stake);
    assert_eq!(prov.status, ProviderSubnetStatus::Active);
    assert_eq!(r.total_providers(), 1);
}

#[test]
fn test_register_provider_insufficient_stake() {
    let mut r = initialized();
    let p = provider(0x02);
    let too_little = isa(999); // Model requires 1 000 ISA
    let err = r
        .register_provider(SubnetId::Model, p, too_little, 10)
        .unwrap_err();
    assert_eq!(
        err,
        SubnetError::InsufficientStake {
            required: isa(1_000),
            provided: too_little,
        }
    );
}

#[test]
fn test_register_provider_subnet_full() {
    let mut r = registry();
    // Create a subnet with max_providers = 1.
    let admin_addr = admin();
    r.register_subnet(
        SubnetConfig {
            id: SubnetId::Market,
            name: "tiny".to_string(),
            description: "capacity-1 subnet".to_string(),
            min_provider_stake: isa(1),
            emission_weight: 500,
            fee_rate_bps: 200,
            max_providers: 1,
            status: SubnetStatus::Active,
            created_at: 0,
        },
        &admin_addr,
    )
    .unwrap();

    r.register_provider(SubnetId::Market, provider(0x01), isa(1), 1)
        .unwrap();
    let err = r
        .register_provider(SubnetId::Market, provider(0x02), isa(1), 1)
        .unwrap_err();
    assert_eq!(err, SubnetError::SubnetFull);
}

#[test]
fn test_remove_provider() {
    let mut r = initialized();
    let p = provider(0x03);
    r.register_provider(SubnetId::Tools, p, isa(500), 5).unwrap();
    assert_eq!(r.total_providers(), 1);

    r.remove_provider(&SubnetId::Tools, &p).unwrap();
    assert_eq!(r.total_providers(), 0);
    assert!(r.get_provider(&SubnetId::Tools, &p).is_none());
}

#[test]
fn test_get_subnet_providers() {
    let mut r = initialized();
    let p1 = provider(0x04);
    let p2 = provider(0x05);
    r.register_provider(SubnetId::Storage, p1, isa(500), 1).unwrap();
    r.register_provider(SubnetId::Storage, p2, isa(600), 2).unwrap();

    let providers = r.get_subnet_providers(&SubnetId::Storage);
    assert_eq!(providers.len(), 2);
}

#[test]
fn test_get_provider_subnets() {
    let mut r = initialized();
    let p = provider(0x06);
    r.register_provider(SubnetId::Model, p, isa(1_000), 1).unwrap();
    r.register_provider(SubnetId::Agent, p, isa(1_000), 1).unwrap();

    let mut subnets = r.get_provider_subnets(&p);
    subnets.sort_by_key(|s| format!("{:?}", s));
    assert_eq!(subnets.len(), 2);
    assert!(subnets.contains(&SubnetId::Model));
    assert!(subnets.contains(&SubnetId::Agent));
}

#[test]
fn test_update_quality_score() {
    let mut r = initialized();
    let p = provider(0x07);
    r.register_provider(SubnetId::Compute, p, isa(2_000), 1).unwrap();

    r.update_quality_score(&SubnetId::Compute, &p, 8500).unwrap();
    assert_eq!(
        r.get_provider(&SubnetId::Compute, &p).unwrap().quality_score,
        8500
    );

    // Score is capped at 10 000.
    r.update_quality_score(&SubnetId::Compute, &p, 99_999).unwrap();
    assert_eq!(
        r.get_provider(&SubnetId::Compute, &p).unwrap().quality_score,
        10_000
    );
}

#[test]
fn test_register_in_paused_subnet_fails() {
    let mut r = initialized();
    r.pause_subnet(&SubnetId::Tools, &admin()).unwrap();
    let err = r
        .register_provider(SubnetId::Tools, provider(0x08), isa(500), 10)
        .unwrap_err();
    assert_eq!(err, SubnetError::SubnetNotActive);
}

#[test]
fn test_duplicate_provider_fails() {
    let mut r = initialized();
    let p = provider(0x09);
    r.register_provider(SubnetId::Agent, p, isa(1_000), 1).unwrap();
    let err = r
        .register_provider(SubnetId::Agent, p, isa(1_000), 2)
        .unwrap_err();
    assert_eq!(err, SubnetError::ProviderAlreadyRegistered);
}
