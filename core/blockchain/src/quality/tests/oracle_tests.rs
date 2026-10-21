use super::helpers::*;
use crate::quality::types::QualityOracleError;
use crate::subnet::SubnetId;

#[test]
fn test_aggregate_score() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);

    for (i, score) in [6000u32, 8000, 7000].iter().enumerate() {
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, *score, i as u64))
            .unwrap();
    }

    // Average of 6000, 8000, 7000 = 7000
    let avg = oracle.aggregate_score(&SubnetId::Model, &p).unwrap();
    assert_eq!(avg, 7000);
    assert_eq!(oracle.get_score(&SubnetId::Model, &p), Some(7000));
}

#[test]
fn test_insufficient_reports() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);

    // Only 2 reports; min is 3.
    oracle
        .submit_report(make_report(p, SubnetId::Model, r, 8000, 1))
        .unwrap();
    oracle
        .submit_report(make_report(p, SubnetId::Model, r, 7000, 2))
        .unwrap();

    let err = oracle.aggregate_score(&SubnetId::Model, &p).unwrap_err();
    assert_eq!(err, QualityOracleError::InsufficientReports);
}

#[test]
fn test_get_top_providers() {
    let (mut oracle, r) = oracle_with_reporter();
    let p1 = provider(0x11);
    let p2 = provider(0x12);
    let p3 = provider(0x13);

    // Give each provider exactly 3 reports so aggregation succeeds.
    for (p, scores) in [
        (p1, [9000u32, 9200, 9100]),
        (p2, [7000, 6800, 7200]),
        (p3, [5000, 5200, 4800]),
    ] {
        for (i, score) in scores.iter().enumerate() {
            oracle
                .submit_report(make_report(p, SubnetId::Tools, r, *score, i as u64))
                .unwrap();
        }
    }

    let top = oracle.get_top_providers(&SubnetId::Tools, 2);
    assert_eq!(top.len(), 2);
    assert_eq!(top[0].0, p1); // highest avg ≈ 9100
    assert_eq!(top[1].0, p2); // second highest avg = 7000
}

#[test]
fn test_multiple_subnets() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);

    // Same provider, two different subnets.
    for i in 0..3 {
        oracle
            .submit_report(make_report(p, SubnetId::Model, r, 9000, i))
            .unwrap();
        oracle
            .submit_report(make_report(p, SubnetId::Storage, r, 5000, i))
            .unwrap();
    }

    assert_eq!(oracle.aggregate_score(&SubnetId::Model, &p).unwrap(), 9000);
    assert_eq!(
        oracle.aggregate_score(&SubnetId::Storage, &p).unwrap(),
        5000
    );

    // Scores don't bleed across subnets.
    assert_ne!(
        oracle.get_score(&SubnetId::Model, &p),
        oracle.get_score(&SubnetId::Storage, &p)
    );
}

#[test]
fn test_provider_count() {
    let (mut oracle, r) = oracle_with_reporter();
    assert_eq!(oracle.provider_count(&SubnetId::Agent), 0);

    let p1 = provider(0x11);
    let p2 = provider(0x12);

    oracle
        .submit_report(make_report(p1, SubnetId::Agent, r, 7000, 1))
        .unwrap();
    assert_eq!(oracle.provider_count(&SubnetId::Agent), 1);

    oracle
        .submit_report(make_report(p2, SubnetId::Agent, r, 8000, 2))
        .unwrap();
    assert_eq!(oracle.provider_count(&SubnetId::Agent), 2);

    // Different subnet — should not affect Agent count.
    oracle
        .submit_report(make_report(p1, SubnetId::Model, r, 9000, 3))
        .unwrap();
    assert_eq!(oracle.provider_count(&SubnetId::Agent), 2);
}

#[test]
fn test_get_reports() {
    let (mut oracle, r) = oracle_with_reporter();
    let p = provider(0x10);

    assert!(oracle.get_reports(&SubnetId::Compute, &p).is_empty());

    oracle
        .submit_report(make_report(p, SubnetId::Compute, r, 7500, 1))
        .unwrap();
    oracle
        .submit_report(make_report(p, SubnetId::Compute, r, 8000, 2))
        .unwrap();

    let reports = oracle.get_reports(&SubnetId::Compute, &p);
    assert_eq!(reports.len(), 2);
    assert_eq!(reports[0].score, 7500);
    assert_eq!(reports[1].score, 8000);
}
