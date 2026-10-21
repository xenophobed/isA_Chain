use super::*;
use crate::subnet::registry::SubnetRegistry;
use crate::subnet::types::{isa, SubnetConfig, SubnetError, SubnetId, SubnetStatus};

#[test]
fn test_initialize_default_subnets() {
    let r = initialized();
    assert_eq!(r.subnet_count(), 6);
    assert!(r.get_subnet(&SubnetId::Model).is_some());
    assert!(r.get_subnet(&SubnetId::Tools).is_some());
    assert!(r.get_subnet(&SubnetId::Compute).is_some());
    assert!(r.get_subnet(&SubnetId::Storage).is_some());
    assert!(r.get_subnet(&SubnetId::Agent).is_some());
    assert!(r.get_subnet(&SubnetId::Market).is_some());
}

#[test]
fn test_emission_weights_sum() {
    let r = initialized();
    let total: u32 = r.get_emission_weights().values().sum();
    assert_eq!(total, 10_000, "emission weights must sum to 10 000 bps");
}

#[test]
fn test_pause_resume_subnet() {
    let mut r = initialized();
    r.pause_subnet(&SubnetId::Tools, &admin()).unwrap();
    assert_eq!(
        r.get_subnet(&SubnetId::Tools).unwrap().status,
        SubnetStatus::Paused
    );

    r.resume_subnet(&SubnetId::Tools, &admin()).unwrap();
    assert_eq!(
        r.get_subnet(&SubnetId::Tools).unwrap().status,
        SubnetStatus::Active
    );
}

#[test]
fn test_unauthorized_admin_fails() {
    let mut r = initialized();
    let bad = other_admin();

    let err = r.pause_subnet(&SubnetId::Model, &bad).unwrap_err();
    assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

    let err = r.resume_subnet(&SubnetId::Model, &bad).unwrap_err();
    assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

    let err = r
        .set_emission_weight(&SubnetId::Model, 1000, &bad)
        .unwrap_err();
    assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

    let dummy_config = SubnetConfig {
        id: SubnetId::Market,
        name: "x".to_string(),
        description: "x".to_string(),
        min_provider_stake: isa(1),
        emission_weight: 500,
        fee_rate_bps: 100,
        max_providers: 10,
        status: SubnetStatus::Active,
        created_at: 0,
    };
    // Use a fresh registry (Market not yet registered) to reach the auth check.
    let mut r2 = SubnetRegistry::new(admin());
    let err = r2.register_subnet(dummy_config, &bad).unwrap_err();
    assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));
}
