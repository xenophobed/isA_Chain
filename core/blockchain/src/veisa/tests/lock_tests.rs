use super::*;

#[test]
fn test_lock() {
    let mut sys = system();
    let owner = addr(0x01);
    let duration = 100_000_u64;
    let height = 1_000_u64;

    let power = sys.lock(owner, isa(10), duration, height).unwrap();
    assert!(power > 0, "voting power should be positive");

    let lock = sys.get_lock(&owner).unwrap();
    assert_eq!(lock.owner, owner);
    assert_eq!(lock.amount, isa(10));
    assert_eq!(lock.lock_start, height);
    assert_eq!(lock.lock_end, height + duration);

    assert_eq!(sys.get_total_locked(), isa(10));
}

#[test]
fn test_extend_lock() {
    let mut sys = system();
    let max = sys.max_lock_duration;
    let owner = addr(0x01);

    // Lock for 50 % of max.
    sys.lock(owner, isa(100), max / 2, 0).unwrap();
    let original_end = sys.get_lock(&owner).unwrap().lock_end;

    // Extend so the new end is at max from block 0.
    let new_end = max;
    let power_after = sys.extend_lock(&owner, new_end, 0).unwrap();

    let lock = sys.get_lock(&owner).unwrap();
    assert_eq!(lock.lock_end, new_end);
    assert!(lock.lock_end > original_end);

    // Power should now equal full amount (locked for max duration).
    assert_eq!(power_after, isa(100));
}

#[test]
fn test_increase_amount() {
    let mut sys = system();
    let max = sys.max_lock_duration;
    let owner = addr(0x01);

    sys.lock(owner, isa(50), max, 0).unwrap();
    let power_before = sys.get_lock(&owner).unwrap().voting_power;

    sys.increase_amount(&owner, isa(50)).unwrap();

    let lock = sys.get_lock(&owner).unwrap();
    assert_eq!(lock.amount, isa(100));
    assert!(lock.voting_power > power_before);
    assert_eq!(sys.get_total_locked(), isa(100));
}

#[test]
fn test_withdraw_after_expiry() {
    let mut sys = system();
    let owner = addr(0x01);
    let duration = sys.min_lock_duration;

    sys.lock(owner, isa(100), duration, 0).unwrap();

    // Withdraw exactly at expiry.
    let withdrawn = sys.withdraw(&owner, duration).unwrap();
    assert_eq!(withdrawn, isa(100));

    // Lock should be gone.
    assert!(sys.get_lock(&owner).is_none());
    assert_eq!(sys.get_total_locked(), 0);
}

#[test]
fn test_withdraw_before_expiry_fails() {
    let mut sys = system();
    let owner = addr(0x01);
    let duration = sys.min_lock_duration;

    sys.lock(owner, isa(100), duration, 0).unwrap();

    // One block before expiry.
    let err = sys.withdraw(&owner, duration - 1).unwrap_err();
    assert_eq!(err, VeISAError::LockNotExpired);

    // Lock should still be intact.
    assert!(sys.get_lock(&owner).is_some());
    assert_eq!(sys.get_total_locked(), isa(100));
}

#[test]
fn test_duplicate_lock_fails() {
    let mut sys = system();
    let owner = addr(0x01);
    let duration = sys.min_lock_duration;

    sys.lock(owner, isa(10), duration, 0).unwrap();

    let err = sys
        .lock(owner, isa(10), duration, 0)
        .unwrap_err();
    assert_eq!(err, VeISAError::LockAlreadyExists(owner));
}

#[test]
fn test_invalid_duration() {
    let mut sys = system();
    let owner_a = addr(0x01);
    let owner_b = addr(0x02);

    // Too short.
    let err = sys
        .lock(owner_a, isa(10), sys.min_lock_duration - 1, 0)
        .unwrap_err();
    assert_eq!(err, VeISAError::InvalidDuration);

    // Too long.
    let err = sys
        .lock(owner_b, isa(10), sys.max_lock_duration + 1, 0)
        .unwrap_err();
    assert_eq!(err, VeISAError::InvalidDuration);
}

#[test]
fn test_zero_amount_fails() {
    let mut sys = system();
    let owner = addr(0x01);
    let duration = sys.min_lock_duration;

    let err = sys.lock(owner, 0, duration, 0).unwrap_err();
    assert_eq!(err, VeISAError::ZeroAmount);
}

#[test]
fn test_no_lock_found() {
    let mut sys = system();
    let ghost = addr(0xFF);

    assert_eq!(sys.get_voting_power(&ghost, 100), 0);
    assert!(sys.get_lock(&ghost).is_none());

    assert_eq!(
        sys.extend_lock(&ghost, 9999, 100).unwrap_err(),
        VeISAError::NoLockFound(ghost)
    );
    assert_eq!(
        sys.increase_amount(&ghost, isa(1)).unwrap_err(),
        VeISAError::NoLockFound(ghost)
    );
    assert_eq!(
        sys.withdraw(&ghost, 999_999).unwrap_err(),
        VeISAError::NoLockFound(ghost)
    );
}
