use super::*;
use crate::agent_registry::{AgentCapability, AgentPricing};
use crate::sdk_types::types::{
    AgentDiscoveryQuery, AgentDiscoveryResult, CompanionWalletConfig, SdkVersion, TradeAgentConfig,
};
use crate::subnet::SubnetId;

#[test]
fn test_agent_discovery_query() {
    let query = AgentDiscoveryQuery {
        capabilities: vec![AgentCapability::TextGeneration, AgentCapability::ToolUse],
        min_reputation: Some(500),
        max_price: Some(1_000),
        limit: 10,
    };
    assert_eq!(query.capabilities.len(), 2);
    assert_eq!(query.min_reputation, Some(500));
    assert_eq!(query.limit, 10);
}

#[test]
fn test_agent_discovery_query_no_filters() {
    let query = AgentDiscoveryQuery {
        capabilities: vec![],
        min_reputation: None,
        max_price: None,
        limit: 20,
    };
    assert!(query.capabilities.is_empty());
    assert!(query.min_reputation.is_none());
    assert!(query.max_price.is_none());
}

#[test]
fn test_agent_discovery_result() {
    let result = AgentDiscoveryResult {
        agent_id: zero_hash(),
        name: "reasoning-agent-v1".to_string(),
        capabilities: vec![AgentCapability::Reasoning],
        reputation: 850,
        pricing: AgentPricing {
            base_fee: 100,
            per_token_fee: 1,
            per_second_fee: 10,
            minimum_charge: 50,
        },
        wallet: zero_addr(),
    };
    assert_eq!(result.name, "reasoning-agent-v1");
    assert_eq!(result.reputation, 850);
    assert_eq!(result.pricing.base_fee, 100);
}

#[test]
fn test_companion_config() {
    let cfg = CompanionWalletConfig {
        owner: zero_addr(),
        companion_agent_id: zero_hash(),
        daily_allowance: 5_000,
        auto_stake: true,
        preferred_subnets: vec![SubnetId::Agent, SubnetId::Model],
    };
    assert_eq!(cfg.daily_allowance, 5_000);
    assert!(cfg.auto_stake);
    assert_eq!(cfg.preferred_subnets.len(), 2);
    assert_eq!(cfg.preferred_subnets[0], SubnetId::Agent);
}

#[test]
fn test_companion_config_no_auto_stake() {
    let cfg = CompanionWalletConfig {
        owner: zero_addr(),
        companion_agent_id: zero_hash(),
        daily_allowance: 1_000,
        auto_stake: false,
        preferred_subnets: vec![],
    };
    assert!(!cfg.auto_stake);
    assert!(cfg.preferred_subnets.is_empty());
}

#[test]
fn test_trade_agent_config() {
    let cfg = TradeAgentConfig {
        agent_id: zero_hash(),
        wallet: zero_addr(),
        trading_pairs: vec![
            ("ISA".to_string(), "USDC".to_string()),
            ("ISA".to_string(), "ETH".to_string()),
        ],
        max_position_size: 100_000,
        stop_loss_bps: 200,
        take_profit_bps: 500,
    };
    assert_eq!(cfg.trading_pairs.len(), 2);
    assert_eq!(cfg.trading_pairs[0], ("ISA".to_string(), "USDC".to_string()));
    assert_eq!(cfg.max_position_size, 100_000);
    assert_eq!(cfg.stop_loss_bps, 200);
    assert_eq!(cfg.take_profit_bps, 500);
}

#[test]
fn test_trade_agent_config_risk_params() {
    let cfg = TradeAgentConfig {
        agent_id: zero_hash(),
        wallet: zero_addr(),
        trading_pairs: vec![],
        max_position_size: 50_000,
        stop_loss_bps: 100,  // 1%
        take_profit_bps: 300, // 3%
    };
    // take-profit must exceed stop-loss for a valid risk/reward ratio
    assert!(cfg.take_profit_bps > cfg.stop_loss_bps);
}

#[test]
fn test_sdk_version() {
    let v = SdkVersion::current();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
    assert_eq!(v.chain_id, 1);
}

#[test]
fn test_sdk_compatibility() {
    let v1 = SdkVersion::current();
    let v2 = SdkVersion {
        major: 0,
        minor: 2,
        patch: 0,
        chain_id: 1,
    };
    let v3 = SdkVersion {
        major: 1,
        minor: 0,
        patch: 0,
        chain_id: 1,
    };
    let v4 = SdkVersion {
        major: 0,
        minor: 1,
        patch: 0,
        chain_id: 2,
    };

    // Same major, same chain — compatible
    assert!(v1.is_compatible(&v2));
    // Different major — incompatible
    assert!(!v1.is_compatible(&v3));
    // Different chain_id — incompatible even with matching major
    assert!(!v1.is_compatible(&v4));
    // Self-compatibility
    assert!(v1.is_compatible(&v1.clone()));
}
