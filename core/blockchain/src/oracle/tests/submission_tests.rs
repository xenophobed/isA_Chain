use super::*;
use crate::oracle::types::OracleError;

#[test]
fn test_submit_price() {
    let mut oracle = setup_oracle();

    // First submission: sets price and records a point
    oracle
        .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
        .unwrap();

    assert_eq!(oracle.get_price().unwrap(), ONE_DOLLAR);
    assert_eq!(oracle.prices.len(), 1);
    assert_eq!(oracle.last_update_height, Some(10));
}

#[test]
fn test_unauthorized_feeder_fails() {
    let mut oracle = setup_oracle();

    let result = oracle.submit_price(ONE_DOLLAR, 10, 1_000_000, &random_addr());
    assert!(matches!(result, Err(OracleError::UnauthorizedFeeder(_))));
}

#[test]
fn test_price_deviation_check() {
    let mut oracle = setup_oracle();

    // Set an initial price
    oracle
        .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
        .unwrap();

    // A 5% move — within the 10% limit
    let within_limit = ONE_DOLLAR + ONE_DOLLAR / 20; // +5%
    assert!(oracle
        .submit_price(within_limit, 11, 1_000_001, &feeder())
        .is_ok());

    // A 15% move — exceeds the 10% limit
    let above_limit = within_limit + within_limit * 15 / 100;
    let result = oracle.submit_price(above_limit, 12, 1_000_002, &feeder());
    assert_eq!(result, Err(OracleError::PriceDeviationTooHigh));
}

#[test]
fn test_price_staleness() {
    let mut oracle = setup_oracle();

    // Submit at height 10
    oracle
        .submit_price(ONE_DOLLAR, 10, 1_000_000, &feeder())
        .unwrap();

    // At height 109 (99 blocks later) — not stale
    assert!(!oracle.is_stale(109));

    // At height 111 (101 blocks later) — stale
    assert!(oracle.is_stale(111));
}

#[test]
fn test_authorize_revoke_feeder() {
    let mut oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);
    let addr = other_feeder();

    // Not yet authorized
    assert!(!oracle.authorized_feeders.contains(&addr));

    // Authorize
    oracle.authorize_feeder(addr, &admin()).unwrap();
    assert!(oracle.authorized_feeders.contains(&addr));

    // Revoke
    oracle.revoke_feeder(&addr, &admin()).unwrap();
    assert!(!oracle.authorized_feeders.contains(&addr));

    // Non-admin cannot authorize
    let result = oracle.authorize_feeder(addr, &random_addr());
    assert!(matches!(result, Err(OracleError::UnauthorizedAdmin(_))));

    // Non-admin cannot revoke
    oracle.authorize_feeder(addr, &admin()).unwrap();
    let result2 = oracle.revoke_feeder(&addr, &random_addr());
    assert!(matches!(result2, Err(OracleError::UnauthorizedAdmin(_))));
}

#[test]
fn test_invalid_price_fails() {
    let mut oracle = setup_oracle();

    let result = oracle.submit_price(0, 10, 1_000_000, &feeder());
    assert_eq!(result, Err(OracleError::InvalidPrice));
}
