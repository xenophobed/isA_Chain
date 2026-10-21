use crate::agent_wallet::*;
use crate::subnet::SubnetId;
use super::{make_agent_id, make_factory, make_owner, DEFAULT_PERIOD, ONE_ISA};

#[test]
fn test_spend_success() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(4);
    let owner = make_owner(0xDD);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA).unwrap();
    factory.spend(&addr, ONE_ISA / 2, 0, None).unwrap();

    let wallet = factory.get_wallet(&addr).unwrap();
    assert_eq!(wallet.balance, ONE_ISA / 2);
    assert_eq!(wallet.spent_this_period, ONE_ISA / 2);
    assert_eq!(wallet.total_spent, ONE_ISA / 2);
}

#[test]
fn test_spend_exceeds_limit() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(5);
    let owner = make_owner(0xEE);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA * 10).unwrap();

    let result = factory.spend(&addr, ONE_ISA + 1, 0, None);
    assert!(matches!(
        result,
        Err(AgentWalletError::SpendingLimitExceeded { .. })
    ));
}

#[test]
fn test_spend_insufficient_balance() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(6);
    let owner = make_owner(0xFF);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, 100).unwrap();

    let result = factory.spend(&addr, 200, 0, None);
    assert_eq!(result, Err(AgentWalletError::InsufficientBalance));
}

#[test]
fn test_spend_frozen_fails() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(7);
    let owner = make_owner(0x11);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA).unwrap();
    factory.freeze(&addr, &owner).unwrap();

    let result = factory.spend(&addr, 100, 0, None);
    assert_eq!(result, Err(AgentWalletError::WalletFrozen));
}

#[test]
fn test_period_reset() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(11);
    let owner = make_owner(0x55);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA * 10).unwrap();

    // Spend up to the limit in period 0
    factory.spend(&addr, ONE_ISA, 0, None).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().spent_this_period,
        ONE_ISA
    );

    // Advance past the period boundary — spend should auto-reset
    let next_period_height = DEFAULT_PERIOD;
    factory.spend(&addr, ONE_ISA / 2, next_period_height, None).unwrap();

    let wallet = factory.get_wallet(&addr).unwrap();
    assert_eq!(wallet.spent_this_period, ONE_ISA / 2);
    assert_eq!(wallet.period_start_height, next_period_height);
}

#[test]
fn test_daily_limit() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(30);
    let owner = make_owner(0xD0);
    let daily_limit = ONE_ISA;
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA * 100, DEFAULT_PERIOD, 0, daily_limit, 0)
        .unwrap();

    factory.deposit(&addr, ONE_ISA * 10).unwrap();

    // Spend up to the daily limit
    factory.spend(&addr, ONE_ISA, 0, None).unwrap();

    // One more spend in the same day should fail
    let result = factory.spend(&addr, 1, 0, None);
    assert!(matches!(
        result,
        Err(AgentWalletError::DailyLimitExceeded { .. })
    ));

    // Advance past one full day — daily counter resets
    let next_day = DAILY_BLOCKS;
    factory.spend(&addr, ONE_ISA / 2, next_day, None).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().daily_spent,
        ONE_ISA / 2
    );
}

#[test]
fn test_monthly_limit() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(31);
    let owner = make_owner(0xD1);
    let monthly_limit = ONE_ISA * 5;
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA * 100, DEFAULT_PERIOD, 0, 0, monthly_limit)
        .unwrap();

    factory.deposit(&addr, ONE_ISA * 20).unwrap();

    // Spend up to the monthly limit
    factory.spend(&addr, ONE_ISA * 5, 0, None).unwrap();

    // One more spend in same month should fail
    let result = factory.spend(&addr, 1, 0, None);
    assert!(matches!(
        result,
        Err(AgentWalletError::MonthlyLimitExceeded { .. })
    ));

    // Advance past one full month — monthly counter resets
    let next_month = MONTHLY_BLOCKS;
    factory.spend(&addr, ONE_ISA, next_month, None).unwrap();
    assert_eq!(
        factory.get_wallet(&addr).unwrap().monthly_spent,
        ONE_ISA
    );
}

#[test]
fn test_allowed_subnets() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(32);
    let owner = make_owner(0xD2);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA * 100, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory
        .get_wallet_mut(&addr)
        .unwrap()
        .allowed_subnets = Some(vec![SubnetId::Model, SubnetId::Tools]);

    factory.deposit(&addr, ONE_ISA * 10).unwrap();

    factory.spend(&addr, ONE_ISA, 0, Some(SubnetId::Model)).unwrap();
    factory.spend(&addr, ONE_ISA, 0, Some(SubnetId::Tools)).unwrap();
    factory.spend(&addr, ONE_ISA, 0, None).unwrap();
}

#[test]
fn test_subnet_not_allowed() {
    let mut factory = make_factory();
    let agent_id = make_agent_id(33);
    let owner = make_owner(0xD3);
    let addr = factory
        .create_wallet(agent_id, owner, ONE_ISA * 100, DEFAULT_PERIOD, 0, 0, 0)
        .unwrap();

    factory
        .get_wallet_mut(&addr)
        .unwrap()
        .allowed_subnets = Some(vec![SubnetId::Model]);

    factory.deposit(&addr, ONE_ISA * 10).unwrap();

    let result = factory.spend(&addr, ONE_ISA, 0, Some(SubnetId::Compute));
    assert_eq!(result, Err(AgentWalletError::SubnetNotAllowed));

    factory.spend(&addr, ONE_ISA, 0, Some(SubnetId::Model)).unwrap();
}
