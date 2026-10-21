use super::helpers::*;

#[test]
fn test_calculate_reward() {
    let c = controller();
    // Block 0 — no halving yet.
    assert_eq!(c.calculate_reward(0), 100 * ISA);
    // Block 999 — still first epoch.
    assert_eq!(c.calculate_reward(999), 100 * ISA);
    // Block 1000 — first halving: 100 >> 1 = 50.
    assert_eq!(c.calculate_reward(1_000), 50 * ISA);
}

#[test]
fn test_halving() {
    let c = controller();
    assert_eq!(c.calculate_reward(0), 100 * ISA);
    assert_eq!(c.calculate_reward(1_000), 50 * ISA);
    assert_eq!(c.calculate_reward(2_000), 25 * ISA);
    assert_eq!(c.calculate_reward(3_000), 12 * ISA + 500_000_000_000_000_000);
}

#[test]
fn test_min_reward_floor() {
    // With a very large height the halved reward must not fall below min_reward.
    let c = controller(); // min = 1 ISA
    let reward = c.calculate_reward(1_000_000);
    assert!(reward >= ISA, "reward {reward} must be >= 1 ISA");
    assert_eq!(reward, ISA); // expected to hit the floor
}

#[test]
fn test_get_current_reward_rate() {
    let c = controller();
    assert_eq!(c.get_current_reward_rate(0), 100 * ISA);
    assert_eq!(c.get_current_reward_rate(1_000), 50 * ISA);
}
