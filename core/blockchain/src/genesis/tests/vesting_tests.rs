use super::helpers::make_vest;

#[test]
fn test_vesting_before_cliff() {
    let vest = make_vest(1_000, 0, 100, 1000);
    // Before the cliff nothing vests.
    assert_eq!(vest.vested_amount(0), 0);
    assert_eq!(vest.vested_amount(50), 0);
    assert_eq!(vest.vested_amount(99), 0);
}

#[test]
fn test_vesting_after_cliff_linear() {
    // Vesting: start=0, cliff=0, end=1000, total=1000
    // After 500 blocks exactly 500 tokens should be vested.
    let vest = make_vest(1_000, 0, 0, 1000);
    assert_eq!(vest.vested_amount(500), 500);
    assert_eq!(vest.vested_amount(250), 250);
    assert_eq!(vest.vested_amount(750), 750);
}

#[test]
fn test_vesting_fully_vested() {
    let vest = make_vest(1_000, 0, 100, 1000);
    assert_eq!(vest.vested_amount(1000), 1_000);
    assert_eq!(vest.vested_amount(2000), 1_000);
    assert!(vest.is_fully_vested(1000));
    assert!(!vest.is_fully_vested(999));
}

#[test]
fn test_vesting_release() {
    let mut vest = make_vest(1_000, 0, 0, 1000);

    // At height 500, 500 tokens are vested — release them.
    let released = vest.release(500);
    assert_eq!(released, 500);
    assert_eq!(vest.released, 500);

    // Releasing again at the same height yields 0 (already released).
    let second = vest.release(500);
    assert_eq!(second, 0);

    // At height 1000 the remaining 500 should become available.
    let final_release = vest.release(1000);
    assert_eq!(final_release, 500);
    assert_eq!(vest.released, 1_000);
}

#[test]
fn test_releasable() {
    let mut vest = make_vest(1_000, 0, 0, 1000);

    // Nothing released yet — all vested tokens are releasable.
    assert_eq!(vest.releasable(500), 500);

    vest.release(500);
    // After releasing, releasable at same height = 0.
    assert_eq!(vest.releasable(500), 0);
    // At height 1000 the remaining 500 are releasable.
    assert_eq!(vest.releasable(1000), 500);
}
