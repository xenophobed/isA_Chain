use super::*;
use crate::settlement::ServiceType;
use crate::types::Amount;

#[test]
fn test_user_total() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 30, "a".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user1(), service_tool(), 20, "b".into(), 470, 2, 0)
        .unwrap();

    assert_eq!(engine.get_user_total(&user1()), 50);
    assert_eq!(engine.get_user_total(&user2()), 0);
}

#[test]
fn test_service_summary() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 40, "x".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user2(), service_model(), 60, "y".into(), 500, 2, 0)
        .unwrap();

    let summary = engine.get_service_summary(&service_model()).unwrap();
    assert_eq!(summary.total_credits, 100);
    assert_eq!(summary.total_requests, 2);
    assert_eq!(summary.average_cost, 50);

    // Unknown service returns None.
    assert!(engine.get_service_summary(&ServiceType::Storage).is_none());
}

#[test]
fn test_all_summaries() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 10, "m".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user1(), service_tool(), 20, "t".into(), 490, 2, 0)
        .unwrap();

    let summaries = engine.get_all_service_summaries();
    assert_eq!(summaries.len(), 2);

    let model_sum: Amount = summaries
        .iter()
        .find(|s| s.service == service_model())
        .map(|s| s.total_credits)
        .unwrap();
    let tool_sum: Amount = summaries
        .iter()
        .find(|s| s.service == service_tool())
        .map(|s| s.total_credits)
        .unwrap();

    assert_eq!(model_sum, 10);
    assert_eq!(tool_sum, 20);
}

#[test]
fn test_multiple_users() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 15, "u1".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user2(), service_model(), 25, "u2".into(), 500, 2, 0)
        .unwrap();

    assert_eq!(engine.get_user_total(&user1()), 15);
    assert_eq!(engine.get_user_total(&user2()), 25);
    assert_eq!(engine.get_user_records(&user1()).len(), 1);
    assert_eq!(engine.get_user_records(&user2()).len(), 1);
}

#[test]
fn test_multiple_services() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 10, "m".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user1(), service_tool(), 5, "t".into(), 490, 2, 0)
        .unwrap();

    assert_eq!(
        engine
            .get_service_summary(&service_model())
            .unwrap()
            .total_credits,
        10
    );
    assert_eq!(
        engine
            .get_service_summary(&service_tool())
            .unwrap()
            .total_credits,
        5
    );
    assert_eq!(engine.total_records(), 2);
}

#[test]
fn test_total_tracking() {
    let mut engine = setup();

    engine
        .charge(user1(), service_model(), 100, "a".into(), 500, 1, 0)
        .unwrap();
    engine
        .charge(user2(), service_tool(), 200, "b".into(), 500, 2, 0)
        .unwrap();

    assert_eq!(engine.total_credits_charged(), 300);
    assert_eq!(engine.total_records(), 2);
}
