use super::*;

#[test]
fn test_lock_voting_power() {
    let mut sys = system();
    let max = sys.max_lock_duration;

    // Locking for the maximum duration should yield power == amount.
    let power_max = sys
        .lock(addr(0x01), isa(100), max, 0)
        .unwrap();
    assert_eq!(power_max, isa(100));

    // Locking for half the maximum duration should yield power == amount / 2.
    let power_half = sys
        .lock(addr(0x02), isa(100), max / 2, 0)
        .unwrap();
    assert_eq!(power_half, isa(50));
}

#[test]
fn test_power_decay() {
    let mut sys = system();
    let max = sys.max_lock_duration;
    let owner = addr(0x01);

    sys.lock(owner, isa(100), max, 0).unwrap();

    // At lock creation, power == amount.
    let power_at_0 = sys.get_voting_power(&owner, 0);
    assert_eq!(power_at_0, isa(100));

    // After half the duration has elapsed, power ≈ amount / 2.
    let power_at_half = sys.get_voting_power(&owner, max / 2);
    assert_eq!(power_at_half, isa(50));

    // After the full duration, power == 0 (lock expired).
    let power_expired = sys.get_voting_power(&owner, max);
    assert_eq!(power_expired, 0);
}

#[test]
fn test_total_voting_power() {
    let mut sys = system();
    let max = sys.max_lock_duration;

    // Two users each lock 100 ISA for max duration — combined power == 200 ISA.
    sys.lock(addr(0x01), isa(100), max, 0).unwrap();
    sys.lock(addr(0x02), isa(100), max, 0).unwrap();

    let total = sys.get_total_voting_power(0);
    assert_eq!(total, isa(200));

    // At half duration, total should be ≈ 100 ISA.
    let total_half = sys.get_total_voting_power(max / 2);
    assert_eq!(total_half, isa(100));
}

#[test]
fn test_max_power_at_max_duration() {
    let mut sys = system();
    let max = sys.max_lock_duration;
    let owner = addr(0x01);
    let amount = isa(500);

    let power = sys.lock(owner, amount, max, 0).unwrap();

    // Locking for the maximum duration must return power == amount.
    assert_eq!(power, amount);

    // get_voting_power at block 0 should also return amount.
    assert_eq!(sys.get_voting_power(&owner, 0), amount);
}
