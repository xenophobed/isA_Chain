use super::*;
use crate::reputation::types::ReputationError;

#[test]
fn test_get_top_agents() {
    let mut sys = default_system();

    // Register 5 agents with different initial scores
    for i in 0u8..5 {
        let id = make_hash(i);
        let mut agent_sys = ReputationSystem::new(make_address(0x01), (i as u32) * 1000);
        agent_sys.register_agent(id).unwrap();
        // Copy the record into our main system
        let rec = agent_sys.records.remove(&id).unwrap();
        sys.records.insert(id, rec);
    }

    let top3 = sys.get_top_agents(3);
    assert_eq!(top3.len(), 3);
    // Sorted by score descending
    assert!(top3[0].1 >= top3[1].1);
    assert!(top3[1].1 >= top3[2].1);
}

#[test]
fn test_decay_inactive() {
    let mut sys = default_system();
    let id = make_hash(0x20);
    sys.register_agent(id).unwrap();

    // last_updated is 0; current_height is 200; threshold is 100 → agent is inactive
    sys.decay_inactive(200, 100);
    let score_after = sys.get_score(&id).unwrap();
    assert_eq!(score_after, 4990); // 5000 - 10 (decay_rate_bps)
}

#[test]
fn test_decay_not_applied_to_active_agents() {
    let mut sys = default_system();
    let id = make_hash(0x21);
    sys.register_agent(id).unwrap();

    // Record a success at block 150 so last_updated = 150
    sys.record_success(&id, 100, 50, 150).unwrap();

    // current_height = 200, threshold = 100 → 200 - 150 = 50 ≤ 100 → not inactive
    sys.decay_inactive(200, 100);
    let score_after = sys.get_score(&id).unwrap();
    assert_eq!(score_after, 5050); // unchanged from the success bump
}

#[test]
fn test_success_rate() {
    let mut sys = default_system();
    let id = make_hash(0x22);
    sys.register_agent(id).unwrap();

    // No interactions yet → None
    assert!(sys.success_rate(&id).is_none());

    sys.record_success(&id, 100, 50, 1).unwrap();
    sys.record_success(&id, 100, 50, 2).unwrap();
    sys.record_failure(&id, 3, "err".to_string()).unwrap();

    // 2 successes out of 3 → 6666 bps (rounded down)
    let rate = sys.success_rate(&id).unwrap();
    assert_eq!(rate, 6666);
}

#[test]
fn test_agent_not_found_errors() {
    let mut sys = default_system();
    let missing = make_hash(0xFF);

    assert_eq!(
        sys.record_success(&missing, 0, 0, 1).unwrap_err(),
        ReputationError::AgentNotFound(missing)
    );
    assert_eq!(
        sys.record_failure(&missing, 1, "x".to_string()).unwrap_err(),
        ReputationError::AgentNotFound(missing)
    );
    assert_eq!(
        sys.record_dispute(&missing, true, 1).unwrap_err(),
        ReputationError::AgentNotFound(missing)
    );
    assert_eq!(
        sys.apply_slash(&missing, 100, 1).unwrap_err(),
        ReputationError::AgentNotFound(missing)
    );
    assert_eq!(
        sys.apply_bonus(&missing, 100, 1, "x".to_string()).unwrap_err(),
        ReputationError::AgentNotFound(missing)
    );
}

#[test]
fn test_authorize_updater_admin_only() {
    let admin = make_address(0x01);
    let mut sys = ReputationSystem::new(admin, 5000);
    let updater = make_address(0x02);
    let impostor = make_address(0xFF);

    // Admin can authorize
    sys.authorize_updater(updater, &admin).unwrap();
    assert!(sys.authorized_updaters.contains(&updater));

    // Non-admin cannot authorize
    let err = sys.authorize_updater(updater, &impostor).unwrap_err();
    assert_eq!(err, ReputationError::UnauthorizedUpdater(impostor));
}
