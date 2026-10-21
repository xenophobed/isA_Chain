use super::*;
use crate::reputation::types::{
    ReputationError, ReputationEventType, DISPUTE_LOSS_DELTA, DISPUTE_WIN_DELTA, MAX_SCORE,
};

#[test]
fn test_register_agent() {
    let mut sys = default_system();
    let id = make_hash(0x10);

    sys.register_agent(id).unwrap();

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.agent_id, id);
    assert_eq!(rec.score, 5000);
    assert_eq!(rec.total_interactions, 0);
}

#[test]
fn test_duplicate_register() {
    let mut sys = default_system();
    let id = make_hash(0x10);

    sys.register_agent(id).unwrap();
    let err = sys.register_agent(id).unwrap_err();
    assert_eq!(err, ReputationError::AlreadyRegistered(id));
}

#[test]
fn test_record_success() {
    let mut sys = default_system();
    let id = make_hash(0x11);
    sys.register_agent(id).unwrap();

    let new_score = sys.record_success(&id, 1000, 200, 10).unwrap();
    assert_eq!(new_score, 5050); // 5000 + 50

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.successful_interactions, 1);
    assert_eq!(rec.total_revenue, 1000);
    assert_eq!(rec.history.len(), 1);
    assert_eq!(rec.history[0].event_type, ReputationEventType::JobCompleted);
}

#[test]
fn test_record_failure() {
    let mut sys = default_system();
    let id = make_hash(0x12);
    sys.register_agent(id).unwrap();

    let new_score = sys
        .record_failure(&id, 10, "timeout".to_string())
        .unwrap();
    assert_eq!(new_score, 4900); // 5000 - 100

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.failed_interactions, 1);
    assert_eq!(rec.history[0].event_type, ReputationEventType::JobFailed);
}

#[test]
fn test_score_capping_at_max() {
    let mut sys = ReputationSystem::new(make_address(0x01), 9990);
    let id = make_hash(0x13);
    sys.register_agent(id).unwrap();

    // +50 would exceed 10 000, should cap
    let new_score = sys.record_success(&id, 0, 0, 1).unwrap();
    assert_eq!(new_score, MAX_SCORE);
    assert!(new_score <= MAX_SCORE);
}

#[test]
fn test_score_floor_at_zero() {
    let mut sys = ReputationSystem::new(make_address(0x01), 50);
    let id = make_hash(0x14);
    sys.register_agent(id).unwrap();

    // -100 from 50 would go negative, should floor at 0
    let new_score = sys
        .record_failure(&id, 1, "network error".to_string())
        .unwrap();
    assert_eq!(new_score, 0);
}

#[test]
fn test_dispute_won() {
    let mut sys = default_system();
    let id = make_hash(0x15);
    sys.register_agent(id).unwrap();

    let new_score = sys.record_dispute(&id, true, 5).unwrap();
    assert_eq!(new_score, 5200); // 5000 + 200

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.history[0].event_type, ReputationEventType::DisputeWon);
    assert_eq!(rec.history[0].delta, DISPUTE_WIN_DELTA);
}

#[test]
fn test_dispute_lost() {
    let mut sys = default_system();
    let id = make_hash(0x16);
    sys.register_agent(id).unwrap();

    let new_score = sys.record_dispute(&id, false, 5).unwrap();
    assert_eq!(new_score, 4700); // 5000 - 300

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.history[0].event_type, ReputationEventType::DisputeLost);
    assert_eq!(rec.history[0].delta, DISPUTE_LOSS_DELTA);
}

#[test]
fn test_apply_slash() {
    let mut sys = default_system();
    let id = make_hash(0x17);
    sys.register_agent(id).unwrap();

    let new_score = sys.apply_slash(&id, 500, 10).unwrap();
    assert_eq!(new_score, 4500); // 5000 - 500

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.history[0].event_type, ReputationEventType::SlashPenalty);
}

#[test]
fn test_apply_bonus() {
    let mut sys = default_system();
    let id = make_hash(0x18);
    sys.register_agent(id).unwrap();

    let new_score = sys
        .apply_bonus(&id, 300, 10, "top performer".to_string())
        .unwrap();
    assert_eq!(new_score, 5300); // 5000 + 300

    let rec = sys.get_reputation(&id).unwrap();
    assert_eq!(rec.history[0].event_type, ReputationEventType::BonusReward);
    assert_eq!(rec.history[0].details, "top performer");
}
