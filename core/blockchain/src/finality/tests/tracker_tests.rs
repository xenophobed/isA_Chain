use super::helpers::*;
use crate::finality::tracker::FinalityTracker;
use crate::finality::types::{FinalityError, FinalityStatus};
use crate::types::BlockHeight;

#[test]
fn test_register_block() {
    let mut tracker = FinalityTracker::new(6667);
    let hash = make_hash(1);
    assert!(tracker.register_block(hash, 1).is_ok());
    let bf = tracker.get_block_finality(&hash).unwrap();
    assert_eq!(bf.height, 1);
    assert_eq!(bf.status, FinalityStatus::Pending);
    assert!(bf.votes.is_empty());
}

#[test]
fn test_add_vote() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    let vote = make_vote(make_address(1), hash, 1, 0);
    let status = tracker.add_vote(vote, 30).unwrap();
    // 30/100 = 30% — below quorum
    assert_eq!(status, FinalityStatus::Pending);

    let bf = tracker.get_block_finality(&hash).unwrap();
    assert_eq!(bf.votes.len(), 1);
    assert_eq!(bf.vote_power, 30);
}

#[test]
fn test_duplicate_vote_rejected() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    let voter = make_address(1);
    let vote1 = make_vote(voter, hash, 1, 0);
    let vote2 = make_vote(voter, hash, 1, 0);

    tracker.add_vote(vote1, 30).unwrap();
    let err = tracker.add_vote(vote2, 30).unwrap_err();
    assert_eq!(err, FinalityError::DuplicateVote { voter, height: 1 });
}

#[test]
fn test_finalize_block() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 5).unwrap();

    // Reach quorum first
    for i in 1u8..=3 {
        let vote = make_vote(make_address(i), hash, 5, 0);
        tracker.add_vote(vote, 25).unwrap();
    }
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Justified);

    // Finalize
    tracker.finalize(&hash).unwrap();
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Finalized);
    assert!(tracker.is_finalized(&hash));
    assert_eq!(tracker.get_finalized_height(), 5);
    assert_eq!(tracker.get_finalized_hash(), hash);
}

#[test]
fn test_already_finalized() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(1);
    tracker.register_block(hash, 1).unwrap();

    // Force justify and finalize
    let vote = make_vote(make_address(1), hash, 1, 0);
    tracker.add_vote(vote, 100).unwrap();
    tracker.finalize(&hash).unwrap();

    // Attempting to finalize again should error
    let err = tracker.finalize(&hash).unwrap_err();
    assert_eq!(err, FinalityError::AlreadyFinalized(hash));

    // Adding a vote to an already-finalized block should also error
    let vote2 = make_vote(make_address(2), hash, 1, 0);
    let err2 = tracker.add_vote(vote2, 10).unwrap_err();
    assert_eq!(err2, FinalityError::AlreadyFinalized(hash));
}

#[test]
fn test_finalized_height_updates() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);

    // Finalize block at height 3
    let hash3 = make_hash(3);
    tracker.register_block(hash3, 3).unwrap();
    let v = make_vote(make_address(1), hash3, 3, 0);
    tracker.add_vote(v, 100).unwrap();
    tracker.finalize(&hash3).unwrap();
    assert_eq!(tracker.get_finalized_height(), 3);

    // Finalize block at height 7 — should update
    let hash7 = make_hash(7);
    tracker.register_block(hash7, 7).unwrap();
    let v2 = make_vote(make_address(2), hash7, 7, 0);
    tracker.add_vote(v2, 100).unwrap();
    tracker.finalize(&hash7).unwrap();
    assert_eq!(tracker.get_finalized_height(), 7);
    assert_eq!(tracker.get_finalized_hash(), hash7);
}

#[test]
fn test_prune_below() {
    let mut tracker = FinalityTracker::new(6667);

    // Register blocks at heights 1–5
    for i in 1u8..=5 {
        let hash = make_hash(i);
        tracker.register_block(hash, i as BlockHeight).unwrap();
    }
    assert_eq!(tracker.blocks.len(), 5);

    // Prune blocks below height 3 (removes heights 1 and 2)
    tracker.prune_below(3);
    assert_eq!(tracker.blocks.len(), 3);

    // Heights 3, 4, 5 should still be present
    for i in 3u8..=5 {
        assert!(tracker.get_block_finality(&make_hash(i)).is_some());
    }
    // Heights 1 and 2 should be gone
    for i in 1u8..=2 {
        assert!(tracker.get_block_finality(&make_hash(i)).is_none());
    }
}

#[test]
fn test_get_status() {
    let mut tracker = FinalityTracker::new(6667);
    let hash = make_hash(1);

    // Unknown block → None
    assert!(tracker.get_status(&hash).is_none());

    tracker.register_block(hash, 1).unwrap();
    assert_eq!(*tracker.get_status(&hash).unwrap(), FinalityStatus::Pending);
}

#[test]
fn test_block_not_found_on_vote() {
    let mut tracker = FinalityTracker::new(6667);
    tracker.set_total_power(100);
    let hash = make_hash(99);

    let vote = make_vote(make_address(1), hash, 1, 0);
    let err = tracker.add_vote(vote, 50).unwrap_err();
    assert_eq!(err, FinalityError::BlockNotFound(hash));
}
