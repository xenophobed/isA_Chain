use crate::agent_token::*;
use super::{linear_curve, make_address, make_hash, quad_curve};

#[test]
fn test_market_cap() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x01);
    let buyer = make_address(0x10);

    factory
        .create_token(agent_id, linear_curve(), make_address(0xAA), 1)
        .unwrap();

    assert_eq!(factory.get_market_cap(&agent_id).unwrap(), 0);

    factory.buy(&agent_id, buyer, 500 * SCALE).unwrap();

    let cap = factory.get_market_cap(&agent_id).unwrap();
    assert!(cap > 0, "Market cap should be positive after buys");
}

#[test]
fn test_token_not_found() {
    let factory = AgentTokenFactory::new();
    let missing = make_hash(0xFF);

    assert_eq!(
        factory.get_price(&missing).unwrap_err(),
        AgentTokenError::TokenNotFound(missing)
    );
    assert_eq!(
        factory.get_market_cap(&missing).unwrap_err(),
        AgentTokenError::TokenNotFound(missing)
    );
    assert!(factory.get_token(&missing).is_none());
    assert_eq!(factory.get_balance(&missing, &make_address(0x01)), 0);
}

#[test]
fn test_quadratic_curve_buy_sell() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x02);
    let buyer = make_address(0x20);

    factory
        .create_token(agent_id, quad_curve(), make_address(0xAA), 1)
        .unwrap();

    let isa_amount = 50 * SCALE;
    let tokens = factory.buy(&agent_id, buyer, isa_amount).unwrap();
    assert!(tokens > 0, "Should receive tokens for quadratic curve buy");

    let reserve_before = factory.get_token(&agent_id).unwrap().reserve_balance;
    let isa_back = factory.sell(&agent_id, &buyer, tokens).unwrap();

    let token = factory.get_token(&agent_id).unwrap();
    assert_eq!(token.total_supply, 0);
    assert_eq!(token.reserve_balance, 0);
    assert_eq!(isa_back, reserve_before);
}

#[test]
fn test_sigmoid_curve() {
    let mut factory = AgentTokenFactory::new();
    let agent_id = make_hash(0x03);
    let buyer = make_address(0x30);

    let sigmoid = BondingCurve::Sigmoid {
        max_price: 1000 * SCALE,
        midpoint: 500,
    };

    factory
        .create_token(agent_id, sigmoid, make_address(0xAA), 1)
        .unwrap();

    let tokens = factory.buy(&agent_id, buyer, 100 * SCALE).unwrap();
    assert!(tokens > 0);

    let token = factory.get_token(&agent_id).unwrap();
    assert!(token.reserve_balance > 0);
    assert_eq!(token.total_supply, tokens);
}

#[test]
fn test_total_tokens_counter() {
    let mut factory = AgentTokenFactory::new();

    assert_eq!(factory.total_tokens(), 0);

    for i in 0u8..5 {
        factory
            .create_token(
                make_hash(i),
                linear_curve(),
                make_address(0xAA),
                i as u64,
            )
            .unwrap();
    }

    assert_eq!(factory.total_tokens(), 5);
}
