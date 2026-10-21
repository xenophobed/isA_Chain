use super::*;
use crate::credits::types::CreditError;

#[test]
fn test_get_balance_no_account() {
    let cs = setup();
    assert_eq!(cs.get_balance(&random_addr()), 0);
    assert!(cs.get_account(&random_addr()).is_none());
}

#[test]
fn test_get_isa_cost_for_credits() {
    // formula: isa_needed = credits * credit_price_usd / isa_price_usd
    //          = credits * 100 / isa_price_usd
    // 5_000_000 credits at $0.50/ISA: 5_000_000 * 100 / 500_000 = 1_000 micro-ISA units
    let cost = CreditSystem::get_isa_cost_for_credits(5_000_000, ISA_PRICE);
    assert_eq!(cost, 1_000);

    // At $1.00/ISA: 10_000_000 credits * 100 / 1_000_000 = 1_000 micro-ISA units
    let cost2 = CreditSystem::get_isa_cost_for_credits(10_000_000, 1_000_000);
    assert_eq!(cost2, 1_000);
}

#[test]
fn test_get_credits_for_isa() {
    // 1_000_000 micro-ISA at $0.50/ISA → 5_000 credits
    // credits = (1_000_000 * 500_000) / (100 * 1_000_000) = 5_000
    let credits = CreditSystem::credits_for_isa(1_000_000, ISA_PRICE);
    assert_eq!(credits, 5_000);

    // 2_000_000 micro-ISA at $1.00/ISA → 20_000 credits
    let credits2 = CreditSystem::credits_for_isa(2_000_000, 1_000_000);
    assert_eq!(credits2, 20_000);

    // Zero ISA → zero credits
    let credits3 = CreditSystem::credits_for_isa(0, ISA_PRICE);
    assert_eq!(credits3, 0);
}

#[test]
fn test_set_credit_price() {
    let mut cs = setup();
    assert_eq!(cs.credit_price_usd, DEFAULT_CREDIT_PRICE_USD);

    cs.set_credit_price(20_000, &admin()).unwrap();
    assert_eq!(cs.credit_price_usd, 20_000);
}

#[test]
fn test_unauthorized_admin_fails() {
    let mut cs = setup();

    let result = cs.set_credit_price(20_000, &random_addr());
    assert_eq!(result, Err(CreditError::UnauthorizedAdmin(random_addr())));

    let result2 = cs.grant_credits(user(), 100, &random_addr(), 1);
    assert_eq!(result2, Err(CreditError::UnauthorizedAdmin(random_addr())));
}

#[test]
fn test_grant_credits() {
    let mut cs = setup();

    cs.grant_credits(user(), 500, &admin(), 5).unwrap();

    assert_eq!(cs.get_balance(&user()), 500);
    assert_eq!(cs.total_credits_issued, 500);

    let acct = cs.get_account(&user()).unwrap();
    assert_eq!(acct.last_top_up_height, 5);
}

#[test]
fn test_total_credits_tracking() {
    let mut cs = setup();

    // Issue 10_000 credits via purchase
    cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
        .unwrap(); // 10_000 credits

    // Grant 50 more
    cs.grant_credits(user2(), 50, &admin(), 2).unwrap();

    assert_eq!(cs.total_credits_issued, 10_050);
    assert_eq!(cs.total_credits_burned, 0);
    assert_eq!(cs.total_credits_in_circulation(), 10_050);

    // Spend 30
    cs.spend_credits(&user(), 30).unwrap();
    assert_eq!(cs.total_credits_burned, 30);
    assert_eq!(cs.total_credits_in_circulation(), 10_020);
}
