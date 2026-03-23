//! Agent Registry — On-Chain Agent Discovery and Management
//!
//! This module provides an on-chain registry for AI agents, allowing them to
//! register their capabilities and pricing, and enabling callers to discover
//! agents by capability.

use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Errors
// ============================================================================

/// Errors for the agent registry
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RegistryError {
    #[error("Agent not found: {0:?}")]
    AgentNotFound(Hash),

    #[error("Agent already registered: {0:?}")]
    AgentAlreadyRegistered(Hash),

    #[error("Unauthorized owner: {0}")]
    UnauthorizedOwner(Address),

    #[error("Unauthorized admin: {0}")]
    UnauthorizedAdmin(Address),

    #[error("Invalid name: agent name must not be empty")]
    InvalidName,

    #[error("No capabilities: agent must have at least one capability")]
    NoCapabilities,

    #[error("Agent is suspended and cannot be modified")]
    AgentSuspended,
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Capabilities an agent can advertise
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentCapability {
    TextGeneration,
    CodeGeneration,
    ImageGeneration,
    DataAnalysis,
    WebBrowsing,
    ToolUse,
    MultiModal,
    Reasoning,
    Custom(String),
}

/// Fee structure for invoking an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentPricing {
    /// Flat per-request fee in credits
    pub base_fee: Amount,
    /// Per-token fee (for LLM agents)
    pub per_token_fee: Amount,
    /// Per-second compute fee
    pub per_second_fee: Amount,
    /// Minimum charge per invocation
    pub minimum_charge: Amount,
}

/// Lifecycle status of a registered agent
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentStatus {
    Active,
    Inactive,
    Suspended,
    Deregistered,
}

/// Full on-chain registration record for an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentRegistration {
    /// Unique agent identifier
    pub agent_id: Hash,
    /// Owner / deployer of this agent
    pub owner: Address,
    /// Agent's own wallet address
    pub wallet: Address,
    /// Human-readable name
    pub name: String,
    /// Description of what the agent does
    pub description: String,
    /// Advertised capabilities
    pub capabilities: Vec<AgentCapability>,
    /// Pricing structure
    pub pricing: AgentPricing,
    /// Reputation score in basis points (0–10 000)
    pub reputation_score: u32,
    /// Total job count
    pub total_jobs: u64,
    /// Successfully completed job count
    pub successful_jobs: u64,
    /// Cumulative revenue earned
    pub total_revenue: Amount,
    /// Current lifecycle status
    pub status: AgentStatus,
    /// Block at which the agent was first registered
    pub registered_at: BlockHeight,
    /// Block of last job or status change
    pub last_active: BlockHeight,
    /// Extensible key-value metadata
    pub metadata: HashMap<String, String>,
}

// ============================================================================
// Registry
// ============================================================================

/// On-chain agent registry
#[derive(Clone, Debug)]
pub struct AgentRegistry {
    /// agent_id → registration
    pub agents: HashMap<Hash, AgentRegistration>,
    /// owner → list of agent_ids
    pub agents_by_owner: HashMap<Address, Vec<Hash>>,
    /// capability → list of agent_ids
    pub agents_by_capability: HashMap<AgentCapability, Vec<Hash>>,
    /// Admin address (can update reputation, suspend, etc.)
    pub admin: Address,
}

impl AgentRegistry {
    // -------------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------------

    pub fn new(admin: Address) -> Self {
        Self {
            agents: HashMap::new(),
            agents_by_owner: HashMap::new(),
            agents_by_capability: HashMap::new(),
            admin,
        }
    }

    // -------------------------------------------------------------------------
    // Registration
    // -------------------------------------------------------------------------

    /// Register a new agent. Fails if the agent_id is already known, the name
    /// is empty, or no capabilities are provided.
    pub fn register(
        &mut self,
        agent_id: Hash,
        owner: Address,
        wallet: Address,
        name: String,
        description: String,
        capabilities: Vec<AgentCapability>,
        pricing: AgentPricing,
        height: BlockHeight,
    ) -> Result<(), RegistryError> {
        if self.agents.contains_key(&agent_id) {
            return Err(RegistryError::AgentAlreadyRegistered(agent_id));
        }
        if name.is_empty() {
            return Err(RegistryError::InvalidName);
        }
        if capabilities.is_empty() {
            return Err(RegistryError::NoCapabilities);
        }

        // Update capability index
        for cap in &capabilities {
            self.agents_by_capability
                .entry(cap.clone())
                .or_default()
                .push(agent_id);
        }

        // Update owner index
        self.agents_by_owner
            .entry(owner)
            .or_default()
            .push(agent_id);

        let registration = AgentRegistration {
            agent_id,
            owner,
            wallet,
            name,
            description,
            capabilities,
            pricing,
            reputation_score: 0,
            total_jobs: 0,
            successful_jobs: 0,
            total_revenue: 0,
            status: AgentStatus::Active,
            registered_at: height,
            last_active: height,
            metadata: HashMap::new(),
        };

        self.agents.insert(agent_id, registration);
        Ok(())
    }

    /// Deregister an agent. Only the owner may do this.
    pub fn deregister(
        &mut self,
        agent_id: &Hash,
        owner: &Address,
    ) -> Result<(), RegistryError> {
        let registration = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::AgentNotFound(*agent_id))?;

        if &registration.owner != owner {
            return Err(RegistryError::UnauthorizedOwner(*owner));
        }

        registration.status = AgentStatus::Deregistered;
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    /// Look up a single agent by id.
    pub fn get_agent(&self, agent_id: &Hash) -> Option<&AgentRegistration> {
        self.agents.get(agent_id)
    }

    /// Return all agents owned by the given address.
    pub fn get_agents_by_owner(&self, owner: &Address) -> Vec<&AgentRegistration> {
        self.agents_by_owner
            .get(owner)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agents.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find all agents that advertise the given capability.
    pub fn search_by_capability(
        &self,
        capability: &AgentCapability,
    ) -> Vec<&AgentRegistration> {
        self.agents_by_capability
            .get(capability)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.agents.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Find agents that possess ALL of the listed capabilities.
    pub fn search_by_capabilities(
        &self,
        capabilities: &[AgentCapability],
    ) -> Vec<&AgentRegistration> {
        if capabilities.is_empty() {
            return Vec::new();
        }
        self.agents
            .values()
            .filter(|reg| {
                capabilities
                    .iter()
                    .all(|cap| reg.capabilities.contains(cap))
            })
            .collect()
    }

    /// Return up to `limit` active agents with the given capability, sorted by
    /// reputation (highest first).
    pub fn get_top_agents(
        &self,
        capability: &AgentCapability,
        limit: usize,
    ) -> Vec<&AgentRegistration> {
        let mut matches: Vec<&AgentRegistration> = self
            .search_by_capability(capability)
            .into_iter()
            .filter(|reg| reg.status == AgentStatus::Active)
            .collect();

        matches.sort_by(|a, b| b.reputation_score.cmp(&a.reputation_score));
        matches.truncate(limit);
        matches
    }

    /// Total number of registered agents (all statuses).
    pub fn total_agents(&self) -> usize {
        self.agents.len()
    }

    /// Number of agents with `Active` status.
    pub fn active_agents(&self) -> usize {
        self.agents
            .values()
            .filter(|r| r.status == AgentStatus::Active)
            .count()
    }

    // -------------------------------------------------------------------------
    // Updates
    // -------------------------------------------------------------------------

    /// Update the pricing for an agent. Only the owner may do this, and the
    /// agent must not be suspended.
    pub fn update_pricing(
        &mut self,
        agent_id: &Hash,
        pricing: AgentPricing,
        owner: &Address,
    ) -> Result<(), RegistryError> {
        let registration = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::AgentNotFound(*agent_id))?;

        if &registration.owner != owner {
            return Err(RegistryError::UnauthorizedOwner(*owner));
        }
        if registration.status == AgentStatus::Suspended {
            return Err(RegistryError::AgentSuspended);
        }

        registration.pricing = pricing;
        Ok(())
    }

    /// Update the lifecycle status of an agent. Only the owner may do this.
    pub fn update_status(
        &mut self,
        agent_id: &Hash,
        status: AgentStatus,
        owner: &Address,
    ) -> Result<(), RegistryError> {
        let registration = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::AgentNotFound(*agent_id))?;

        if &registration.owner != owner {
            return Err(RegistryError::UnauthorizedOwner(*owner));
        }

        registration.status = status;
        Ok(())
    }

    /// Record the outcome of a completed job and accumulate stats.
    pub fn record_job(
        &mut self,
        agent_id: &Hash,
        success: bool,
        revenue: Amount,
    ) -> Result<(), RegistryError> {
        let registration = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::AgentNotFound(*agent_id))?;

        registration.total_jobs += 1;
        if success {
            registration.successful_jobs += 1;
        }
        registration.total_revenue += revenue;
        Ok(())
    }

    /// Update an agent's reputation score. Admin-only.
    ///
    /// `score` must be in 0–10 000 basis points.
    pub fn update_reputation(
        &mut self,
        agent_id: &Hash,
        score: u32,
        admin: &Address,
    ) -> Result<(), RegistryError> {
        if admin != &self.admin {
            return Err(RegistryError::UnauthorizedAdmin(*admin));
        }

        let registration = self
            .agents
            .get_mut(agent_id)
            .ok_or_else(|| RegistryError::AgentNotFound(*agent_id))?;

        registration.reputation_score = score.min(10_000);
        Ok(())
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers -----------------------------------------------------------------

    fn make_address(seed: u8) -> Address {
        Address::new([seed; 20])
    }

    fn make_hash(seed: u8) -> Hash {
        Hash::new([seed; 32])
    }

    fn default_pricing() -> AgentPricing {
        AgentPricing {
            base_fee: 100,
            per_token_fee: 1,
            per_second_fee: 10,
            minimum_charge: 50,
        }
    }

    fn register_basic(
        registry: &mut AgentRegistry,
        agent_id: Hash,
        owner: Address,
    ) -> Result<(), RegistryError> {
        registry.register(
            agent_id,
            owner,
            make_address(0xAA),
            "TestAgent".to_string(),
            "A test agent".to_string(),
            vec![AgentCapability::TextGeneration],
            default_pricing(),
            1,
        )
    }

    // Tests -------------------------------------------------------------------

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

        // Both have TextGeneration
        let text_only = registry.search_by_capabilities(&[AgentCapability::TextGeneration]);
        assert_eq!(text_only.len(), 2);

        // Only multi-cap agent has both
        let both = registry.search_by_capabilities(&[
            AgentCapability::TextGeneration,
            AgentCapability::CodeGeneration,
        ]);
        assert_eq!(both.len(), 1);
        assert_eq!(both[0].agent_id, multi_id);

        // Empty capability list returns empty
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

        // Score is capped at 10 000
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
        // Sorted descending by reputation
        assert!(top3[0].reputation_score >= top3[1].reputation_score);
        assert!(top3[1].reputation_score >= top3[2].reputation_score);
        // Highest reputation is agent 4 (score 4000)
        assert_eq!(top3[0].agent_id, make_hash(4));
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

        // Deregister one → should not count as active
        registry
            .deregister(&make_hash(0), &owner)
            .unwrap();

        assert_eq!(registry.total_agents(), 4);
        assert_eq!(registry.active_agents(), 3);
    }
}
