use super::*;
use crate::treasury::types::TreasuryError;

#[test]
fn test_distribute() {
    let mut t = default_treasury();
    t.deposit(1_000).unwrap();

    let recipients = vec![(staker(0x01), 600), (staker(0x02), 400)];
    let dist = t.distribute(recipients.clone(), 42, &admin()).unwrap();

    assert_eq!(dist.total_amount, 1_000);
    assert_eq!(dist.height, 42);
    assert_eq!(dist.recipients, recipients);
    assert_eq!(t.get_balance(), 0);
    assert_eq!(t.get_total_distributed(), 1_000);
}

#[test]
fn test_distribute_insufficient_balance() {
    let mut t = default_treasury();
    t.deposit(500).unwrap();

    let result = t.distribute(vec![(staker(0x01), 1_000)], 1, &admin());
    assert_eq!(result, Err(TreasuryError::InsufficientBalance));
}

#[test]
fn test_distribute_no_recipients_fails() {
    let mut t = default_treasury();
    t.deposit(1_000).unwrap();

    let result = t.distribute(vec![], 1, &admin());
    assert_eq!(result, Err(TreasuryError::NoRecipients));
}

#[test]
fn test_distribute_proportional() {
    let mut t = default_treasury();
    t.deposit(1_000).unwrap();

    // Staker A has 3x the stake of staker B → 750 vs 250
    let stakers = vec![(staker(0x01), 3_000), (staker(0x02), 1_000)];
    let dist = t.distribute_proportional(&stakers, 1_000, 10, &admin()).unwrap();

    assert_eq!(dist.recipients.len(), 2);
    // Staker A: 1000 * 3000 / 4000 = 750
    assert_eq!(dist.recipients[0], (staker(0x01), 750));
    // Staker B: 1000 * 1000 / 4000 = 250
    assert_eq!(dist.recipients[1], (staker(0x02), 250));
    assert_eq!(dist.total_amount, 1_000);
    assert_eq!(t.get_balance(), 0);
}

#[test]
fn test_unauthorized_admin_fails() {
    let mut t = default_treasury();
    t.deposit(1_000).unwrap();

    // distribute with wrong admin
    let result = t.distribute(vec![(staker(0x01), 100)], 1, &other());
    assert_eq!(result, Err(TreasuryError::UnauthorizedAdmin(other())));

    // set_fee_rate with wrong admin
    let result2 = t.set_fee_rate(100, &other());
    assert_eq!(result2, Err(TreasuryError::UnauthorizedAdmin(other())));

    // distribute_proportional with wrong admin
    let result3 = t.distribute_proportional(
        &[(staker(0x01), 1_000)],
        500,
        1,
        &other(),
    );
    assert_eq!(result3, Err(TreasuryError::UnauthorizedAdmin(other())));
}

#[test]
fn test_lifetime_tracking() {
    let mut t = default_treasury();

    // Collect fees
    t.collect_fee(40_000).unwrap(); // fee = 1_000
    t.collect_fee(80_000).unwrap(); // fee = 2_000
    t.deposit(500).unwrap();

    assert_eq!(t.get_total_collected(), 3_500);
    assert_eq!(t.get_balance(), 3_500);

    // Distribute some
    t.distribute(vec![(staker(0x01), 1_000)], 1, &admin()).unwrap();
    assert_eq!(t.get_total_distributed(), 1_000);
    assert_eq!(t.get_balance(), 2_500);

    // Distribute more
    t.distribute(vec![(staker(0x02), 2_000)], 2, &admin()).unwrap();
    assert_eq!(t.get_total_distributed(), 3_000);
    assert_eq!(t.get_balance(), 500);

    // Distribution history
    assert_eq!(t.get_distributions().len(), 2);
    assert_eq!(t.get_distributions()[0].height, 1);
    assert_eq!(t.get_distributions()[1].height, 2);
}

#[test]
fn test_distribute_with_split() {
    let mut t = default_treasury();
    // Deposit 10_000 into treasury
    t.deposit(10_000).unwrap();
    assert_eq!(t.get_balance(), 10_000);

    // Two equal stakers
    let stakers = vec![(staker(0x01), 1_000), (staker(0x02), 1_000)];
    let (staker_dist, treasury_retention) =
        t.distribute_with_split(&stakers, 50, &admin()).unwrap();

    // 40% of 10_000 = 4_000 goes to stakers, split equally: 2_000 each
    assert_eq!(staker_dist.total_amount, 4_000);
    assert_eq!(staker_dist.height, 50);
    assert_eq!(staker_dist.recipients.len(), 2);
    assert_eq!(staker_dist.recipients[0], (staker(0x01), 2_000));
    assert_eq!(staker_dist.recipients[1], (staker(0x02), 2_000));

    // 60% of 10_000 = 6_000 retained
    assert_eq!(treasury_retention.total_amount, 6_000);
    assert_eq!(treasury_retention.recipients[0].0, admin());

    // Balance is reduced only by staker payout; 6_000 stays
    assert_eq!(t.get_balance(), 6_000);
    assert_eq!(t.get_total_distributed(), 4_000);
}
