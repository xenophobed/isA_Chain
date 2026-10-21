use super::*;
use crate::credit_billing::types::CreditBillingError;
use crate::settlement::ServiceType;

#[test]
fn test_charge_success() {
    let mut engine = setup();
    let record = engine
        .charge(user1(), service_model(), 50, "inference".into(), 1_000, 1, 1_000)
        .unwrap();

    assert_eq!(record.user, user1());
    assert_eq!(record.service, service_model());
    assert_eq!(record.credits_charged, 50);
    assert_eq!(record.height, 1);
    assert_eq!(record.timestamp, 1_000);
    assert_eq!(engine.total_records(), 1);
}

#[test]
fn test_insufficient_credits() {
    let mut engine = setup();
    let err = engine
        .charge(user1(), service_model(), 200, "inference".into(), 100, 1, 1_000)
        .unwrap_err();

    assert_eq!(
        err,
        CreditBillingError::InsufficientCredits {
            available: 100,
            required: 200
        }
    );
    assert_eq!(engine.total_records(), 0);
}

#[test]
fn test_service_not_enabled() {
    let mut engine = setup();
    let err = engine
        .charge(
            user1(),
            ServiceType::Storage,
            10,
            "storage".into(),
            1_000,
            1,
            1_000,
        )
        .unwrap_err();

    assert_eq!(err, CreditBillingError::ServiceNotEnabled);
}

#[test]
fn test_rate_limit() {
    // Engine with max 3 requests per period of 100 blocks.
    let mut engine = CreditBillingEngine::new(3, 100);
    engine.enable_service(service_model());

    for i in 0..3u64 {
        engine
            .charge(user1(), service_model(), 1, "req".into(), 1_000, i, 0)
            .expect("should succeed");
    }

    // 4th request in same period should fail.
    let err = engine
        .charge(user1(), service_model(), 1, "req".into(), 1_000, 3, 0)
        .unwrap_err();
    assert_eq!(err, CreditBillingError::RateLimitExceeded);
}

#[test]
fn test_rate_limit_reset() {
    let mut engine = CreditBillingEngine::new(2, 100);
    engine.enable_service(service_model());

    // Exhaust the limit in period starting at block 0.
    engine
        .charge(user1(), service_model(), 1, "r1".into(), 1_000, 0, 0)
        .unwrap();
    engine
        .charge(user1(), service_model(), 1, "r2".into(), 1_000, 1, 0)
        .unwrap();

    // Verify limit is hit.
    assert_eq!(
        engine.charge(user1(), service_model(), 1, "r3".into(), 1_000, 2, 0).unwrap_err(),
        CreditBillingError::RateLimitExceeded
    );

    // Move to a new period (block 100+).
    let record = engine
        .charge(user1(), service_model(), 1, "r4".into(), 1_000, 100, 0)
        .unwrap();
    assert_eq!(record.height, 100);
}

#[test]
fn test_invalid_amount() {
    let mut engine = setup();
    let err = engine
        .charge(user1(), service_model(), 0, "zero".into(), 500, 1, 0)
        .unwrap_err();
    assert_eq!(err, CreditBillingError::InvalidAmount);
}

#[test]
fn test_disable_service() {
    let mut engine = setup();

    // Works before disable.
    engine
        .charge(user1(), service_model(), 10, "ok".into(), 500, 1, 0)
        .unwrap();

    engine.disable_service(&service_model());

    let err = engine
        .charge(user1(), service_model(), 10, "fail".into(), 500, 2, 0)
        .unwrap_err();
    assert_eq!(err, CreditBillingError::ServiceNotEnabled);
}
