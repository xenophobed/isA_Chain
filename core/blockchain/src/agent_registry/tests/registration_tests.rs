use crate::agent_registry::*;
use super::{default_pricing, make_address, make_hash, register_basic};

#[test]
fn test_register_agent() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    assert!(register_basic(&mut registry, agent_id, owner).is_ok());
    let reg = registry.get_agent(&agent_id).unwrap();
    assert_eq!(reg.name, "TestAgent");
    assert_eq!(reg.status, AgentStatus::Active);
    assert_eq!(reg.reputation_score, 0);
    assert_eq!(reg.total_jobs, 0);
}

#[test]
fn test_register_duplicate_fails() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    register_basic(&mut registry, agent_id, owner).unwrap();
    let err = register_basic(&mut registry, agent_id, owner).unwrap_err();
    assert_eq!(err, RegistryError::AgentAlreadyRegistered(agent_id));
}

#[test]
fn test_register_no_capabilities_fails() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);

    let err = registry
        .register(
            make_hash(0x10),
            make_address(0x02),
            make_address(0xAA),
            "Agent".to_string(),
            "desc".to_string(),
            vec![],
            default_pricing(),
            1,
        )
        .unwrap_err();

    assert_eq!(err, RegistryError::NoCapabilities);
}

#[test]
fn test_register_empty_name_fails() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);

    let err = registry
        .register(
            make_hash(0x10),
            make_address(0x02),
            make_address(0xAA),
            String::new(),
            "desc".to_string(),
            vec![AgentCapability::TextGeneration],
            default_pricing(),
            1,
        )
        .unwrap_err();

    assert_eq!(err, RegistryError::InvalidName);
}

#[test]
fn test_deregister() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);

    register_basic(&mut registry, agent_id, owner).unwrap();
    registry.deregister(&agent_id, &owner).unwrap();

    let reg = registry.get_agent(&agent_id).unwrap();
    assert_eq!(reg.status, AgentStatus::Deregistered);
}

#[test]
fn test_unauthorized_owner_fails() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let agent_id = make_hash(0x10);
    let owner = make_address(0x02);
    let impostor = make_address(0xFF);

    register_basic(&mut registry, agent_id, owner).unwrap();

    let err = registry.deregister(&agent_id, &impostor).unwrap_err();
    assert_eq!(err, RegistryError::UnauthorizedOwner(impostor));

    let err = registry
        .update_pricing(&agent_id, default_pricing(), &impostor)
        .unwrap_err();
    assert_eq!(err, RegistryError::UnauthorizedOwner(impostor));
}

#[test]
fn test_get_agents_by_owner() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let owner_a = make_address(0x0A);
    let owner_b = make_address(0x0B);

    let id1 = make_hash(0x01);
    let id2 = make_hash(0x02);
    let id3 = make_hash(0x03);

    register_basic(&mut registry, id1, owner_a).unwrap();

    registry
        .register(
            id2,
            owner_a,
            make_address(0xAB),
            "AgentA2".to_string(),
            "desc".to_string(),
            vec![AgentCapability::DataAnalysis],
            default_pricing(),
            2,
        )
        .unwrap();

    register_basic(&mut registry, id3, owner_b).unwrap();

    let a_agents = registry.get_agents_by_owner(&owner_a);
    assert_eq!(a_agents.len(), 2);

    let b_agents = registry.get_agents_by_owner(&owner_b);
    assert_eq!(b_agents.len(), 1);
    assert_eq!(b_agents[0].agent_id, id3);

    let unknown = make_address(0xFF);
    let none = registry.get_agents_by_owner(&unknown);
    assert!(none.is_empty());
}

#[test]
fn test_active_and_total_agents() {
    let admin = make_address(0x01);
    let mut registry = AgentRegistry::new(admin);
    let owner = make_address(0x02);

    for i in 0u8..4 {
        register_basic(&mut registry, make_hash(i), owner).unwrap();
    }

    assert_eq!(registry.total_agents(), 4);
    assert_eq!(registry.active_agents(), 4);

    registry
        .deregister(&make_hash(0), &owner)
        .unwrap();

    assert_eq!(registry.total_agents(), 4);
    assert_eq!(registry.active_agents(), 3);
}
