use super::*;
use crate::settlement_fallback::types::FallbackError;
use crate::types::Hash;

#[test]
fn test_resolve_deferred() {
    let mut m = manager();

    let rec = m
        .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 100)
        .unwrap();

    // Resolve before deadline (100 + 1_000 = 1_100)
    m.resolve_deferred(&rec.id, 500).unwrap();

    let updated = m.get_record(&rec.id).unwrap();
    assert!(updated.resolved);
    assert_eq!(updated.resolved_at, Some(500));

    // Should no longer be in the deferred list.
    assert!(m.get_pending_deferred().is_empty());

    // Recovery tracking.
    assert_eq!(m.get_total_recovered(), REQUIRED);
}

#[test]
fn test_expire_deferred() {
    let mut m = manager();

    let rec = m
        .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 10)
        .unwrap();

    // Deadline = 10 + 1_000 = 1_010.  Expire at height 1_011.
    let expired = m.expire_deferred(1_011);
    assert_eq!(expired.len(), 1);
    assert_eq!(expired[0], rec.id);

    // The record is still in `records` but removed from `deferred`.
    assert!(m.get_pending_deferred().is_empty());
    assert!(m.get_record(&rec.id).is_some());
}

#[test]
fn test_already_resolved() {
    let mut m = manager();

    let rec = m
        .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 5)
        .unwrap();

    m.resolve_deferred(&rec.id, 100).unwrap();

    let result = m.resolve_deferred(&rec.id, 200);
    assert_eq!(result, Err(FallbackError::AlreadyResolved(rec.id)));
}

#[test]
fn test_recovery_rate() {
    let mut m = manager();

    // Record two shortfalls of 1_000_000 each → total 2_000_000.
    let rec1 = m
        .handle_shortfall(Hash::hash_data(b"s1"), user(), provider(), REQUIRED, 0, 0, 1)
        .unwrap();

    let rec2 = m
        .handle_shortfall(Hash::hash_data(b"s2"), user(), provider(), REQUIRED, 0, 0, 2)
        .unwrap();

    // Resolve only the first.
    m.resolve_deferred(&rec1.id, 10).unwrap();

    // recovered = REQUIRED (5_000_000), shortfall = 2 * REQUIRED (10_000_000)
    // rate = 5_000_000 / 10_000_000 * 10_000 = 5_000 bps
    assert_eq!(m.get_recovery_rate_bps(), 5_000);

    // Resolve the second.
    m.resolve_deferred(&rec2.id, 20).unwrap();
    assert_eq!(m.get_recovery_rate_bps(), 10_000);
}

#[test]
fn test_total_tracking() {
    let mut m = manager();

    m.handle_shortfall(
        Hash::hash_data(b"s1"),
        user(),
        provider(),
        3_000_000,
        1_000_000, // shortfall = 2_000_000
        0,
        1,
    )
    .unwrap();

    m.handle_shortfall(
        Hash::hash_data(b"s2"),
        user(),
        provider(),
        2_000_000,
        0, // shortfall = 2_000_000
        0,
        2,
    )
    .unwrap();

    assert_eq!(m.get_total_shortfall(), 4_000_000);
    assert_eq!(m.get_total_recovered(), 0);
    assert_eq!(m.get_recovery_rate_bps(), 0);
}

#[test]
fn test_get_user_records() {
    let mut m = manager();

    let user2 = Address::from([0x33; 20]);

    // Two records for `user()`.
    m.handle_shortfall(Hash::hash_data(b"s1"), user(), provider(), REQUIRED, 0, 0, 1)
        .unwrap();
    m.handle_shortfall(Hash::hash_data(b"s2"), user(), provider(), REQUIRED, 0, 0, 2)
        .unwrap();

    // One record for `user2`.
    m.handle_shortfall(Hash::hash_data(b"s3"), user2, provider(), REQUIRED, 0, 0, 3)
        .unwrap();

    let recs = m.get_user_records(&user());
    assert_eq!(recs.len(), 2);
    for r in &recs {
        assert_eq!(r.user, user());
    }

    let recs2 = m.get_user_records(&user2);
    assert_eq!(recs2.len(), 1);
}

#[test]
fn test_record_not_found() {
    let mut m = manager();
    let unknown = Hash::hash_data(b"does_not_exist");
    let result = m.resolve_deferred(&unknown, 100);
    assert_eq!(result, Err(FallbackError::RecordNotFound(unknown)));
}

#[test]
fn test_deadline_expired() {
    let mut m = manager();

    let rec = m
        .handle_shortfall(settlement_id(), user(), provider(), REQUIRED, 0, 0, 10)
        .unwrap();

    // deadline = 10 + 1_000 = 1_010; try to resolve at 1_011 (past deadline)
    let result = m.resolve_deferred(&rec.id, 1_011);
    assert_eq!(result, Err(FallbackError::DeadlineExpired));
}
