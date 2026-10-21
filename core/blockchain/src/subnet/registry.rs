use super::types::{
    isa, ProviderSubnetStatus, SubnetConfig, SubnetError, SubnetId, SubnetProvider, SubnetStatus,
};
use crate::types::{Address, Amount, BlockHeight};
use std::collections::HashMap;

// ============================================================================
// SubnetRegistry
// ============================================================================

pub struct SubnetRegistry {
    pub subnets: HashMap<SubnetId, SubnetConfig>,
    /// Keyed by (subnet, provider_address).
    pub providers: HashMap<(SubnetId, Address), SubnetProvider>,
    /// Which subnets each provider address participates in.
    pub provider_subnets: HashMap<Address, Vec<SubnetId>>,
    pub admin: Address,
}

impl SubnetRegistry {
    // ----------------------------------------------------------------
    // Constructor
    // ----------------------------------------------------------------

    pub fn new(admin: Address) -> Self {
        SubnetRegistry {
            subnets: HashMap::new(),
            providers: HashMap::new(),
            provider_subnets: HashMap::new(),
            admin,
        }
    }

    // ----------------------------------------------------------------
    // Subnet management
    // ----------------------------------------------------------------

    /// Populate the registry with the 6 default subnets.
    pub fn initialize_default_subnets(&mut self, height: BlockHeight) -> Result<(), SubnetError> {
        let defaults: Vec<SubnetConfig> = vec![
            SubnetConfig {
                id: SubnetId::Model,
                name: "isA_Model".to_string(),
                description: "Inference subnet — model serving and completions".to_string(),
                min_provider_stake: isa(1_000),
                emission_weight: 2500,
                fee_rate_bps: 250,
                max_providers: 1000,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Tools,
                name: "isA_MCP".to_string(),
                description: "Tool execution subnet — MCP tool servers".to_string(),
                min_provider_stake: isa(500),
                emission_weight: 1500,
                fee_rate_bps: 250,
                max_providers: 1000,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Compute,
                name: "isA_OS".to_string(),
                description: "Compute subnet — VMs, browsers, REPL environments".to_string(),
                min_provider_stake: isa(2_000),
                emission_weight: 2000,
                fee_rate_bps: 250,
                max_providers: 500,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Storage,
                name: "isA_Data".to_string(),
                description: "Storage subnet — persistent data and object store".to_string(),
                min_provider_stake: isa(500),
                emission_weight: 1000,
                fee_rate_bps: 250,
                max_providers: 1000,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Agent,
                name: "isA_Agent".to_string(),
                description: "Agent runtime subnet — orchestration and execution".to_string(),
                min_provider_stake: isa(1_000),
                emission_weight: 2000,
                fee_rate_bps: 250,
                max_providers: 500,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Market,
                name: "isA_Trade".to_string(),
                description: "Marketplace subnet — listings, bids, settlement".to_string(),
                min_provider_stake: isa(500),
                emission_weight: 1000,
                fee_rate_bps: 250,
                max_providers: 500,
                status: SubnetStatus::Active,
                created_at: height,
            },
        ];

        for config in defaults {
            self.register_subnet(config, &self.admin.clone())?;
        }

        Ok(())
    }

    /// Register a new subnet.  Admin-only.
    pub fn register_subnet(
        &mut self,
        config: SubnetConfig,
        caller: &Address,
    ) -> Result<(), SubnetError> {
        if caller != &self.admin {
            return Err(SubnetError::UnauthorizedAdmin(*caller));
        }
        if self.subnets.contains_key(&config.id) {
            return Err(SubnetError::SubnetAlreadyExists);
        }
        self.subnets.insert(config.id, config);
        Ok(())
    }

    /// Look up a subnet config by id.
    pub fn get_subnet(&self, id: &SubnetId) -> Option<&SubnetConfig> {
        self.subnets.get(id)
    }

    // ----------------------------------------------------------------
    // Provider management
    // ----------------------------------------------------------------

    /// Register a provider in a subnet.
    pub fn register_provider(
        &mut self,
        subnet_id: SubnetId,
        address: Address,
        stake: Amount,
        height: BlockHeight,
    ) -> Result<(), SubnetError> {
        // Subnet must exist and be active.
        let subnet = self
            .subnets
            .get(&subnet_id)
            .ok_or(SubnetError::SubnetNotFound)?;

        if subnet.status != SubnetStatus::Active {
            return Err(SubnetError::SubnetNotActive);
        }

        // Enforce minimum stake.
        if stake < subnet.min_provider_stake {
            return Err(SubnetError::InsufficientStake {
                required: subnet.min_provider_stake,
                provided: stake,
            });
        }

        // Enforce capacity cap.
        let current_count = self
            .providers
            .keys()
            .filter(|(sid, _)| sid == &subnet_id)
            .count();
        if current_count >= subnet.max_providers {
            return Err(SubnetError::SubnetFull);
        }

        // No duplicate registrations.
        if self.providers.contains_key(&(subnet_id, address)) {
            return Err(SubnetError::ProviderAlreadyRegistered);
        }

        self.providers.insert(
            (subnet_id, address),
            SubnetProvider {
                address,
                subnet_id,
                stake,
                quality_score: 5000, // start at 50%
                jobs_completed: 0,
                revenue_earned: 0,
                registered_at: height,
                status: ProviderSubnetStatus::Active,
            },
        );

        self.provider_subnets
            .entry(address)
            .or_default()
            .push(subnet_id);

        Ok(())
    }

    /// Deregister a provider from a subnet.
    pub fn remove_provider(
        &mut self,
        subnet_id: &SubnetId,
        address: &Address,
    ) -> Result<(), SubnetError> {
        if self.providers.remove(&(*subnet_id, *address)).is_none() {
            return Err(SubnetError::ProviderNotFound);
        }

        if let Some(subnets) = self.provider_subnets.get_mut(address) {
            subnets.retain(|s| s != subnet_id);
        }

        Ok(())
    }

    pub fn get_provider(
        &self,
        subnet_id: &SubnetId,
        address: &Address,
    ) -> Option<&SubnetProvider> {
        self.providers.get(&(*subnet_id, *address))
    }

    pub fn get_subnet_providers(&self, subnet_id: &SubnetId) -> Vec<&SubnetProvider> {
        self.providers
            .iter()
            .filter_map(|((sid, _), p)| if sid == subnet_id { Some(p) } else { None })
            .collect()
    }

    pub fn get_provider_subnets(&self, address: &Address) -> Vec<SubnetId> {
        self.provider_subnets
            .get(address)
            .cloned()
            .unwrap_or_default()
    }

    // ----------------------------------------------------------------
    // Quality scores
    // ----------------------------------------------------------------

    pub fn update_quality_score(
        &mut self,
        subnet_id: &SubnetId,
        address: &Address,
        score: u32,
    ) -> Result<(), SubnetError> {
        let provider = self
            .providers
            .get_mut(&(*subnet_id, *address))
            .ok_or(SubnetError::ProviderNotFound)?;
        provider.quality_score = score.min(10_000);
        Ok(())
    }

    // ----------------------------------------------------------------
    // Emission weights
    // ----------------------------------------------------------------

    /// Return the emission weight for every registered subnet.
    pub fn get_emission_weights(&self) -> HashMap<SubnetId, u32> {
        self.subnets
            .iter()
            .map(|(id, cfg)| (*id, cfg.emission_weight))
            .collect()
    }

    /// Update the emission weight for one subnet.  Admin-only.
    /// Does NOT validate that all weights still sum to 10 000 — the caller
    /// is responsible for issuing a consistent set of updates.
    pub fn set_emission_weight(
        &mut self,
        subnet_id: &SubnetId,
        weight: u32,
        caller: &Address,
    ) -> Result<(), SubnetError> {
        if caller != &self.admin {
            return Err(SubnetError::UnauthorizedAdmin(*caller));
        }
        let subnet = self
            .subnets
            .get_mut(subnet_id)
            .ok_or(SubnetError::SubnetNotFound)?;
        subnet.emission_weight = weight;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Subnet lifecycle
    // ----------------------------------------------------------------

    pub fn pause_subnet(
        &mut self,
        subnet_id: &SubnetId,
        caller: &Address,
    ) -> Result<(), SubnetError> {
        if caller != &self.admin {
            return Err(SubnetError::UnauthorizedAdmin(*caller));
        }
        let subnet = self
            .subnets
            .get_mut(subnet_id)
            .ok_or(SubnetError::SubnetNotFound)?;
        subnet.status = SubnetStatus::Paused;
        Ok(())
    }

    pub fn resume_subnet(
        &mut self,
        subnet_id: &SubnetId,
        caller: &Address,
    ) -> Result<(), SubnetError> {
        if caller != &self.admin {
            return Err(SubnetError::UnauthorizedAdmin(*caller));
        }
        let subnet = self
            .subnets
            .get_mut(subnet_id)
            .ok_or(SubnetError::SubnetNotFound)?;
        subnet.status = SubnetStatus::Active;
        Ok(())
    }

    // ----------------------------------------------------------------
    // Aggregate queries
    // ----------------------------------------------------------------

    pub fn total_providers(&self) -> usize {
        self.providers.len()
    }

    pub fn subnet_count(&self) -> usize {
        self.subnets.len()
    }
}
