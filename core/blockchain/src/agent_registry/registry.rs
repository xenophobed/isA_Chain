use std::collections::HashMap;
use crate::types::{Address, Amount, BlockHeight, Hash};
use super::types::*;

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
    #[allow(clippy::too_many_arguments)]
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
            .ok_or(RegistryError::AgentNotFound(*agent_id))?;

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
            .ok_or(RegistryError::AgentNotFound(*agent_id))?;

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
            .ok_or(RegistryError::AgentNotFound(*agent_id))?;

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
            .ok_or(RegistryError::AgentNotFound(*agent_id))?;

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
            .ok_or(RegistryError::AgentNotFound(*agent_id))?;

        registration.reputation_score = score.min(10_000);
        Ok(())
    }
}
