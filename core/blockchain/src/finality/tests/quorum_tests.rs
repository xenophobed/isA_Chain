use super::helpers::*;
use crate::finality::tracker::FinalityTracker;
use crate::finality::types::FinalityStatus;

#[test]
fn test_quorum_reached() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    // Need ≥ 66.67% — add three voters totalling 70
    for i in 1u8..=3 {
        let vote = make_vote(make_address(i), hash, 1, 0);
        tracker.add_vote(vote, 23 + i as u128).unwrap(); // 24+25+26 = 75
    }

    let status = tracker.get_status(&hash).unwrap();
    assert_eq!(*status, FinalityStatus::Justified);
    assert!(tracker.check_quorum(&hash));
}

#[test]
fn test_quorum_not_reached() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    // Add only 50% power
    let vote = make_vote(make_address(1), hash, 1, 0);
    tracker.add_vote(vote, 50).unwrap();

    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
    assert!(!tracker.check_quorum(&hash));
}

#[test]
fn test_quorum_threshold_math() {
    // Verify boundary: exactly 6667 bps (66.67%)
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(10_000);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    // 6666 / 10000 = 6666 bps — just below quorum
    let v1 = make_vote(make_address(1), hash, 1, 0);
    tracker.add_vote(v1, 6666).unwrap();
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);

    // Add 1 more unit → 6667 / 10000 = 6667 bps — exactly at quorum
    let v2 = make_vote(make_address(2), hash, 1, 0);
    tracker.add_vote(v2, 1).unwrap();
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Justified);
}

#[test]
fn test_zero_total_power_no_quorum() {
    let mut tracker = FinalityTracker::new(6667);
    // total_validator_power left at 0
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();
    let vote = make_vote(make_address(1), hash, 1, 0);
    tracker.add_vote(vote, 1000).unwrap();
    // With total_power == 0 quorum can never be met
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
    assert!(!tracker.check_quorum(&hash));
}
