use super::helpers::*;
use crate::quality::oracle::ProviderQualityOracle;
use crate::quality::types::QualityOracleError;
use crate::subnet::SubnetId;

#[test]
fn test_submit_report() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);
    let report = make_report(p, SubnetId::Model, r, 8000, 1);
    oracle.submit_report(report).unwrap();
    assert_eq!(oracle.get_reports(&SubnetId::Model, &p).len(), 1);
}

#[test]
fn test_unauthorized_reporter() {
    let (mut oracle, _) = oracle_with_reporter();
    let bad = reporter(0xFF);
    let p = provider(0x10);
    let report = make_report(p, SubnetId::Model, bad, 8000, 1);
    let err = oracle.submit_report(report).unwrap_err();
    assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
}

#[test]
fn test_invalid_score() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);
    let report = make_report(p, SubnetId::Model, r, 10_001, 1);
    let err = oracle.submit_report(report).unwrap_err();
    assert_eq!(err, QualityOracleError::InvalidScore);
}

#[test]
fn test_authorize_revoke() {
    let mut oracle = ProviderQualityOracle::new(admin(), 1);
    let r = reporter(0x01);

    // Not yet authorized.
    let p = provider(0x10);
    let report = make_report(p, SubnetId::Model, r, 8000, 1);
    assert_eq!(
        oracle.submit_report(report).unwrap_err(),
        QualityOracleError::UnauthorizedReporter(r)
    );

    // Authorize.
    oracle.authorize_reporter(r, &admin()).unwrap();
    oracle
        .submit_report(make_report(p, SubnetId::Model, r, 8000, 2))
        .unwrap();

    // Revoke.
    oracle.revoke_reporter(&r, &admin()).unwrap();
    assert_eq!(
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, 8000, 3))
            .unwrap_err(),
        QualityOracleError::UnauthorizedReporter(r)
    );
}

#[test]
fn test_authorize_requires_admin() {
    let mut oracle = ProviderQualityOracle::new(admin(), 1);
    let bad = reporter(0xBB);
    let r = reporter(0x01);
    let err = oracle.authorize_reporter(r, &bad).unwrap_err();
    assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
}

#[test]
fn test_revoke_requires_admin() {
    let mut oracle = ProviderQualityOracle::new(admin(), 1);
    let r = reporter(0x01);
    oracle.authorize_reporter(r, &admin()).unwrap();
    let bad = reporter(0xBB);
    let err = oracle.revoke_reporter(&r, &bad).unwrap_err();
    assert_eq!(err, QualityOracleError::UnauthorizedReporter(bad));
}
