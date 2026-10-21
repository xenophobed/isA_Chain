use crate::agent_token::*;
use super::{linear_curve, make_address, make_hash};

#[test]
fn test_create_token() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let creator = make_address(0xAA);

    factory
        .create_token(agent_id, linear_curve(), creator, 1)
        .unwrap();

    let token = factory.get_token(&agent_id).unwrap();
    assert_eq!(token.agent_id, agent_id);
    assert_eq!(token.total_supply, 0);
    assert_eq!(token.reserve_balance, 0);
    assert_eq!(token.creator, creator);
    assert_eq!(token.created_at, 1);
    assert_eq!(factory.total_tokens(), 1);
}

#[test]
fn test_duplicate_token_fails() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let creator = make_address(0xAA);

    factory
        .create_token(agent_id, linear_curve(), creator, 1)
        .unwrap();

    let err = factory
        .create_token(agent_id, linear_curve(), creator, 2)
        .unwrap_err();

    assert_eq!(err, AgentTokenError::TokenAlreadyExists(agent_id));
}

#[test]
fn test_buy_tokens() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let tokens_received = factory.buy(&agent_id, buyer, 50 * SCALE).unwrap();
    assert!(tokens_received > 0, "Should receive tokens for ISA spend");

    let balance = factory.get_balance(&agent_id, &buyer);
    assert_eq!(balance, tokens_received);

    let token = factory.get_token(&agent_id).unwrap();
    assert_eq!(token.total_supply, tokens_received);
    assert!(token.reserve_balance > 0);
}

#[test]
fn test_sell_tokens() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let isa_spent = 100 * SCALE;
    let tokens = factory.buy(&agent_id, buyer, isa_spent).unwrap();

    let reserve_before = factory.get_token(&agent_id).unwrap().reserve_balance;

    let isa_received = factory.sell(&agent_id, &buyer, tokens).unwrap();

    let token = factory.get_token(&agent_id).unwrap();
    assert_eq!(token.total_supply, 0);
    assert_eq!(token.reserve_balance, 0);
    assert_eq!(factory.get_balance(&agent_id, &buyer), 0);
    assert_eq!(isa_received, reserve_before);
    let delta = isa_spent - isa_received;
    assert!(
        delta * 10_000 <= isa_spent,
        "Rounding loss too large: spent={isa_spent}, received={isa_received}"
    );
}

#[test]
fn test_price_increases_with_supply() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let price_before = factory.get_price(&agent_id).unwrap();

    factory.buy(&agent_id, buyer, 1000 * SCALE).unwrap();

    let price_after = factory.get_price(&agent_id).unwrap();

    assert!(
        price_after > price_before,
        "Price should rise after buying: before={price_before}, after={price_after}"
    );
}

#[test]
fn test_sell_returns_less_than_buy() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let isa_spent = 500 * SCALE;
    let tokens = factory.buy(&agent_id, buyer, isa_spent).unwrap();

    let buyer2 = make_address(0x11);
    factory.buy(&agent_id, buyer2, 1000 * SCALE).unwrap();

    let token_before = factory.get_token(&agent_id).unwrap().reserve_balance;
    let isa_out = factory.sell(&agent_id, &buyer, tokens).unwrap();
    assert!(isa_out <= token_before, "Can't pay out more than reserve");
}

#[test]
fn test_insufficient_funds_zero_amount() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let err = factory.buy(&agent_id, buyer, 0).unwrap_err();
    assert_eq!(err, AgentTokenError::ZeroAmount);
}

#[test]
fn test_zero_amount_sell() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let seller = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    factory.buy(&agent_id, seller, 100 * SCALE).unwrap();

    let err = factory.sell(&agent_id, &seller, 0).unwrap_err();
    assert_eq!(err, AgentTokenError::ZeroAmount);
}

#[test]
fn test_insufficient_tokens_on_sell() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);
    let other = make_address(0x11);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let tokens = factory.buy(&agent_id, buyer, 100 * SCALE).unwrap();

    let err = factory.sell(&agent_id, &other, tokens).unwrap_err();
    assert_eq!(err, AgentTokenError::InsufficientTokens);
}

#[test]
fn test_multiple_buyers() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    let b1 = make_address(0x01);
    let b2 = make_address(0x02);
    let b3 = make_address(0x03);

    let t1 = factory.buy(&agent_id, b1, 100 * SCALE).unwrap();
    let t2 = factory.buy(&agent_id, b2, 200 * SCALE).unwrap();
    let t3 = factory.buy(&agent_id, b3, 300 * SCALE).unwrap();

    assert_eq!(factory.get_balance(&agent_id, &b1), t1);
    assert_eq!(factory.get_balance(&agent_id, &b2), t2);
    assert_eq!(factory.get_balance(&agent_id, &b3), t3);

    let token = factory.get_token(&agent_id).unwrap();
    assert_eq!(token.total_supply, t1 + t2 + t3);
    assert_eq!(token.holders.len(), 3);

    assert!(t1 > t2, "Later buyers should receive fewer tokens");
    assert!(t2 > t3);
}
