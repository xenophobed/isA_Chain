use crate::agent_registry::*;
use super::{default_pricing, make_address, make_hash, register_basic};

#[test]
fn test_search_by_capability() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);

    let id1 = make_hash(0x01);
    let id2 = make_hash(0x02);
    let owner = make_address(0x03);

    registry
        .register(
            id1,
            owner,
            make_address(0xAA),
            "Agent1".to_string(),
            "desc".to_string(),
            vec![AgentCapability::TextGeneration, AgentCapability::ToolUse],
            default_pricing(),
            1,
        )
        .unwrap();

    registry
        .register(
            id2,
            owner,
            make_address(0xBB),
            "Agent2".to_string(),
            "desc".to_string(),
            vec![AgentCapability::CodeGeneration],
            default_pricing(),
            2,
        )
        .unwrap();

    let text_agents = registry.search_by_capability(&AgentCapability::TextGeneration);
    assert_eq!(text_agents.len(), 1);
    assert_eq!(text_agents[0].agent_id, id1);

    let code_agents = registry.search_by_capability(&AgentCapability::CodeGeneration);
    assert_eq!(code_agents.len(), 1);
    assert_eq!(code_agents[0].agent_id, id2);

    let image_agents = registry.search_by_capability(&AgentCapability::ImageGeneration);
    assert!(image_agents.is_empty());
}

#[test]
fn test_search_by_capabilities() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let owner = make_address(0x03);

    let multi_id = make_hash(0x01);
    let single_id = make_hash(0x02);

    registry
        .register(
            multi_id,
            owner,
            make_address(0xAA),
            "Multi".to_string(),
            "desc".to_string(),
            vec![
                AgentCapability::TextGeneration,
                AgentCapability::CodeGeneration,
            ],
            default_pricing(),
            1,
        )
        .unwrap();

    registry
        .register(
            single_id,
            owner,
            make_address(0xBB),
            "Single".to_string(),
            "desc".to_string(),
            vec![AgentCapability::TextGeneration],
            default_pricing(),
            2,
        )
        .unwrap();

    let text_only = registry.search_by_capabilities(&[AgentCapability::TextGeneration]);
    assert_eq!(text_only.len(), 2);

    let both = registry.search_by_capabilities(&[
        AgentCapability::TextGeneration,
        AgentCapability::CodeGeneration,
    ]);
    assert_eq!(both.len(), 1);
    assert_eq!(both[0].agent_id, multi_id);

    let empty = registry.search_by_capabilities(&[]);
    assert!(empty.is_empty());
}

#[test]
fn test_update_pricing() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    register_basic(&mut registry, agent_id, owner).unwrap();

    let new_pricing = AgentPricing {
        base_fee: 500,
        per_token_fee: 5,
        per_second_fee: 50,
        minimum_charge: 200,
    };

    registry
        .update_pricing(&agent_id, new_pricing, &owner)
        .unwrap();

    let reg = registry.get_agent(&agent_id).unwrap();
    assert_eq!(reg.pricing.base_fee, 500);
}

#[test]
fn test_record_job() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    register_basic(&mut registry, agent_id, owner).unwrap();

    registry.record_job(&agent_id, true, 1000).unwrap();
    registry.record_job(&agent_id, false, 0).unwrap();
    registry.record_job(&agent_id, true, 500).unwrap();

    let reg = registry.get_agent(&agent_id).unwrap();
    assert_eq!(reg.total_jobs, 3);
    assert_eq!(reg.successful_jobs, 2);
    assert_eq!(reg.total_revenue, 1500);
}

#[test]
fn test_update_reputation() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    register_basic(&mut registry, agent_id, owner).unwrap();

    registry.update_reputation(&agent_id, 8500, &admin).unwrap();
    assert_eq!(
        registry.get_agent(&agent_id).unwrap().reputation_score,
        8500
    );

    registry
        .update_reputation(&agent_id, 99_999, &admin)
        .unwrap();
    assert_eq!(
        registry.get_agent(&agent_id).unwrap().reputation_score,
        10_000
    );
}

#[test]
fn test_get_top_agents() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let owner = make_address(0x03);

    for i in 0u8..5 {
        let id = make_hash(i);
        registry
            .register(
                id,
                owner,
                make_address(0xA0 + i),
                format!("Agent{i}"),
                "desc".to_string(),
                vec![AgentCapability::Reasoning],
                default_pricing(),
                i as u64,
            )
            .unwrap();
        registry
            .update_reputation(&id, (i as u32) * 1000, &admin)
            .unwrap();
    }

    let top3 = registry.get_top_agents(&AgentCapability::Reasoning, 3);
    assert_eq!(top3.len(), 3);
    assert!(top3[0].reputation_score >= top3[1].reputation_score);
    assert!(top3[1].reputation_score >= top3[2].reputation_score);
    assert_eq!(top3[0].agent_id, make_hash(4));
}
