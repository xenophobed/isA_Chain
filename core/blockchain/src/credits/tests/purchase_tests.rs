use super::*;
use crate::credits::types::CreditError;

#[test]
fn test_purchase_credits() {
    let mut cs = setup();

    // Purchase with 2 ISA at $0.50/ISA
    // credits = (2_000_000 * 500_000) / (100 * 1_000_000) = 10_000 credits
    // min_purchase = 100, so well above the minimum — should succeed
    let isa_amount: Amount = 2_000_000; // 2 ISA in micro-ISA
    let credits = cs
        .purchase_credits(user(), isa_amount, ISA_PRICE, 10)
        .unwrap();

    assert_eq!(credits, 10_000);
    assert_eq!(cs.get_balance(&user()), 10_000);
    assert_eq!(cs.total_credits_issued, 10_000);
}

#[test]
fn test_purchase_credits_conversion_math() {
    let mut cs = setup();

    // 2 ISA at $0.50/ISA → 10_000 credits
    let credits = cs
        .purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
        .unwrap();
    assert_eq!(credits, 10_000);

    // 10 ISA at $1.00/ISA → 100_000 credits
    let credits2 = cs
        .purchase_credits(user2(), 10_000_000, 1_000_000, 2)
        .unwrap();
    assert_eq!(credits2, 100_000);
}

#[test]
fn test_spend_credits() {
    let mut cs = setup();
    cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
        .unwrap(); // 10_000 credits

    cs.spend_credits(&user(), 30).unwrap();

    assert_eq!(cs.get_balance(&user()), 9_970);
    assert_eq!(cs.total_credits_burned, 30);

    let acct = cs.get_account(&user()).unwrap();
    assert_eq!(acct.total_credits_spent, 30);
}

#[test]
fn test_spend_insufficient_fails() {
    let mut cs = setup();
    cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 1)
        .unwrap(); // 10_000 credits

    let result = cs.spend_credits(&user(), 20_000);
    assert_eq!(result, Err(CreditError::InsufficientCredits));

    // Balance must be unchanged
    assert_eq!(cs.get_balance(&user()), 10_000);
}

#[test]
fn test_below_minimum_purchase() {
    let mut cs = setup();

    // With min_purchase = 100, ISA price = $0.50, credit_price = 100 micro-USD
    // credits = (isa_amount * 500_000) / (100 * 1_000_000) = isa_amount / 200
    // Need at least 100 credits → isa_amount >= 20_000 micro-ISA
    // Send only 10_000 micro-ISA → 50 credits < 100 minimum
    let result = cs.purchase_credits(user(), 10_000, ISA_PRICE, 1);
    assert_eq!(result, Err(CreditError::BelowMinimumPurchase));
}

#[test]
fn test_multiple_purchases() {
    let mut cs = setup();

    // First purchase: 2 ISA → 10_000 credits
    cs.purchase_credits(user(), 2_000_000, ISA_PRICE, 10)
        .unwrap();

    // Second purchase: 4 ISA → 20_000 credits
    cs.purchase_credits(user(), 4_000_000, ISA_PRICE, 20)
        .unwrap();

    let acct = cs.get_account(&user()).unwrap();
    assert_eq!(acct.credit_balance, 30_000);
    assert_eq!(acct.total_credits_purchased, 30_000);
    assert_eq!(acct.last_top_up_height, 20);
    assert_eq!(cs.total_credits_issued, 30_000);
}

#[test]
fn test_spend_account_not_found() {
    let mut cs = setup();
    let result = cs.spend_credits(&random_addr(), 10);
    assert_eq!(result, Err(CreditError::AccountNotFound(random_addr())));
}

#[test]
fn test_invalid_amount_purchase() {
    let mut cs = setup();
    let result = cs.purchase_credits(user(), 0, ISA_PRICE, 1);
    assert_eq!(result, Err(CreditError::InvalidAmount));
}
