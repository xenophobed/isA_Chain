use super::*;
use crate::oracle::types::OracleError;

#[test]
fn test_convert_isa_to_usd() {
    let mut oracle = setup_oracle();
    oracle
        .submit_price(ONE_DOLLAR, 1, 1_000_000, &feeder())
        .unwrap();

    // 1 ISA (1_000_000 micro-ISA) at $1.00/ISA → $1.00 (1_000_000 micro-USD)
    let result = oracle.convert_isa_to_usd(1_000_000).unwrap();
    assert_eq!(result, 1_000_000);

    // 2_000_000 micro-ISA at $1.00 → 2_000_000 micro-USD
    let result2 = oracle.convert_isa_to_usd(2_000_000).unwrap();
    assert_eq!(result2, 2_000_000);
}

#[test]
fn test_convert_usd_to_isa() {
    let mut oracle = setup_oracle();
    oracle
        .submit_price(ONE_DOLLAR, 1, 1_000_000, &feeder())
        .unwrap();

    // $1.00 (1_000_000 micro-USD) at $1.00/ISA → 1_000_000 micro-ISA
    let result = oracle.convert_usd_to_isa(1_000_000).unwrap();
    assert_eq!(result, 1_000_000);

    // $2.00 → 2_000_000 micro-ISA
    let result2 = oracle.convert_usd_to_isa(2_000_000).unwrap();
    assert_eq!(result2, 2_000_000);
}

#[test]
fn test_twap_calculation() {
    let mut oracle = setup_oracle();

    // Submit 3 price points: 1_000_000, 1_050_000, 1_100_000
    oracle
        .submit_price(1_000_000, 10, 10_000, &feeder())
        .unwrap();
    oracle
        .submit_price(1_050_000, 11, 11_000, &feeder())
        .unwrap();
    oracle
        .submit_price(1_100_000, 12, 12_000, &feeder())
        .unwrap();

    // TWAP of last 3 points = (1_000_000 + 1_050_000 + 1_100_000) / 3 = 1_050_000
    let twap = oracle.get_twap(3).unwrap();
    assert_eq!(twap, 1_050_000);

    // TWAP of last 2 points = (1_050_000 + 1_100_000) / 2 = 1_075_000
    let twap2 = oracle.get_twap(2).unwrap();
    assert_eq!(twap2, 1_075_000);
}

#[test]
fn test_get_price_at_height() {
    let mut oracle = setup_oracle();

    oracle
        .submit_price(1_000_000, 10, 10_000, &feeder())
        .unwrap();
    oracle
        .submit_price(1_050_000, 20, 20_000, &feeder())
        .unwrap();

    // Exact height match
    let point = oracle.get_price_at(10).unwrap();
    assert_eq!(point.price, 1_000_000);
    assert_eq!(point.height, 10);

    // Height between two points — should return the earlier one
    let point2 = oracle.get_price_at(15).unwrap();
    assert_eq!(point2.price, 1_000_000);

    // Height at or after the latest point
    let point3 = oracle.get_price_at(20).unwrap();
    assert_eq!(point3.price, 1_050_000);

    // Height before any submission
    assert!(oracle.get_price_at(5).is_none());
}

#[test]
fn test_no_price_fails() {
    let oracle = PriceOracle::new(ONE_DOLLAR, admin(), 100, 1000);

    // No submissions yet — get_price should fail
    assert_eq!(oracle.get_price(), Err(OracleError::NoPrice));

    // Conversion should also fail
    assert_eq!(oracle.convert_isa_to_usd(1_000_000), Err(OracleError::NoPrice));
    assert_eq!(oracle.convert_usd_to_isa(1_000_000), Err(OracleError::NoPrice));

    // TWAP should fail
    assert_eq!(oracle.get_twap(5), Err(OracleError::NoPrice));

    // is_stale should return true with no submissions
    assert!(oracle.is_stale(0));
}
