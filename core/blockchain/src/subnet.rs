use crate::types::{Address, Amount, BlockHeight};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// SubnetId
// ============================================================================

/// The 6 isA service subnets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SubnetId {
    /// isA_Model — inference workloads
    Model,
    /// isA_MCP — tool execution
    Tools,
    /// isA_OS — compute / VMs
    Compute,
    /// isA_Data — storage
    Storage,
    /// isA_Agent — agent runtime
    Agent,
    /// isA_Trade — marketplace
    Market,
}

// ============================================================================
// SubnetStatus / ProviderSubnetStatus
// ============================================================================

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubnetStatus {
    Active,
    Paused,
    Deprecated,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderSubnetStatus {
    Active,
    Inactive,
    Slashed,
}

// ============================================================================
// SubnetConfig
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetConfig {
    pub id: SubnetId,
    pub name: String,
    pub description: String,
    /// Minimum stake a provider must post to join this subnet (in ISA wei).
    pub min_provider_stake: Amount,
    /// Fraction of block rewards flowing to this subnet, in basis points.
    /// All active subnets must sum to 10 000.
    pub emission_weight: u32,
    /// Subnet-specific protocol fee, in basis points.
    pub fee_rate_bps: u32,
    /// Hard cap on registered providers.
    pub max_providers: usize,
    pub status: SubnetStatus,
    pub created_at: BlockHeight,
}

// ============================================================================
// SubnetProvider
// ============================================================================

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SubnetProvider {
    pub address: Address,
    pub subnet_id: SubnetId,
    pub stake: Amount,
    /// Quality score in basis points (0 – 10 000).
    pub quality_score: u32,
    pub jobs_completed: u64,
    pub revenue_earned: Amount,
    pub registered_at: BlockHeight,
    pub status: ProviderSubnetStatus,
}

// ============================================================================
// SubnetError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SubnetError {
    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Subnet already exists")]
    SubnetAlreadyExists,

    #[error("Provider not found")]
    ProviderNotFound,

    #[error("Provider already registered in subnet")]
    ProviderAlreadyRegistered,

    #[error("Insufficient stake: required {required}, provided {provided}")]
    InsufficientStake { required: Amount, provided: Amount },

    #[error("Subnet is at maximum provider capacity")]
    SubnetFull,

    #[error("Subnet is not active")]
    SubnetNotActive,

    #[error("Unauthorized admin: {0}")]
    UnauthorizedAdmin(Address),

    #[error("Invalid emission weights: must sum to 10000")]
    InvalidEmissionWeights,
}

// ============================================================================
// Default stake constants (ISA wei — 1 ISA = 10^18 wei)
// ============================================================================

const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

fn isa(n: u128) -> Amount {
    n * ISA
}

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
                emission_weight: 3000,
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
                emission_weight: 2000,
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
                emission_weight: 2500,
                fee_rate_bps: 300,
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
                fee_rate_bps: 200,
                max_providers: 1000,
                status: SubnetStatus::Active,
                created_at: height,
            },
            SubnetConfig {
                id: SubnetId::Agent,
                name: "isA_Agent".to_string(),
                description: "Agent runtime subnet — orchestration and execution".to_string(),
                min_provider_stake: isa(1_000),
                emission_weight: 1000,
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
                emission_weight: 500,
                fee_rate_bps: 200,
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
            .or_insert_with(Vec::new)
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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn other_admin() -> Address {
        Address::from([0xBB; 20])
    }

    fn provider(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn registry() -> SubnetRegistry {
        SubnetRegistry::new(admin())
    }

    fn initialized() -> SubnetRegistry {
        let mut r = registry();
        r.initialize_default_subnets(1).unwrap();
        r
    }

    // ----------------------------------------------------------------

    #[test]
    fn test_initialize_default_subnets() {
        let r = initialized();
        assert_eq!(r.subnet_count(), 6);
        assert!(r.get_subnet(&SubnetId::Model).is_some());
        assert!(r.get_subnet(&SubnetId::Tools).is_some());
        assert!(r.get_subnet(&SubnetId::Compute).is_some());
        assert!(r.get_subnet(&SubnetId::Storage).is_some());
        assert!(r.get_subnet(&SubnetId::Agent).is_some());
        assert!(r.get_subnet(&SubnetId::Market).is_some());
    }

    #[test]
    fn test_emission_weights_sum() {
        let r = initialized();
        let total: u32 = r.get_emission_weights().values().sum();
        assert_eq!(total, 10_000, "emission weights must sum to 10 000 bps");
    }

    #[test]
    fn test_register_provider() {
        let mut r = initialized();
        let p = provider(0x01);
        let stake = isa(1_000); // exactly the Model minimum
        r.register_provider(SubnetId::Model, p, stake, 10).unwrap();

        let prov = r.get_provider(&SubnetId::Model, &p).unwrap();
        assert_eq!(prov.address, p);
        assert_eq!(prov.stake, stake);
        assert_eq!(prov.status, ProviderSubnetStatus::Active);
        assert_eq!(r.total_providers(), 1);
    }

    #[test]
    fn test_register_provider_insufficient_stake() {
        let mut r = initialized();
        let p = provider(0x02);
        let too_little = isa(999); // Model requires 1 000 ISA
        let err = r
            .register_provider(SubnetId::Model, p, too_little, 10)
            .unwrap_err();
        assert_eq!(
            err,
            SubnetError::InsufficientStake {
                required: isa(1_000),
                provided: too_little,
            }
        );
    }

    #[test]
    fn test_register_provider_subnet_full() {
        let mut r = registry();
        // Create a subnet with max_providers = 1.
        let admin_addr = admin();
        r.register_subnet(
            SubnetConfig {
                id: SubnetId::Market,
                name: "tiny".to_string(),
                description: "capacity-1 subnet".to_string(),
                min_provider_stake: isa(1),
                emission_weight: 500,
                fee_rate_bps: 200,
                max_providers: 1,
                status: SubnetStatus::Active,
                created_at: 0,
            },
            &admin_addr,
        )
        .unwrap();

        r.register_provider(SubnetId::Market, provider(0x01), isa(1), 1)
            .unwrap();
        let err = r
            .register_provider(SubnetId::Market, provider(0x02), isa(1), 1)
            .unwrap_err();
        assert_eq!(err, SubnetError::SubnetFull);
    }

    #[test]
    fn test_remove_provider() {
        let mut r = initialized();
        let p = provider(0x03);
        r.register_provider(SubnetId::Tools, p, isa(500), 5).unwrap();
        assert_eq!(r.total_providers(), 1);

        r.remove_provider(&SubnetId::Tools, &p).unwrap();
        assert_eq!(r.total_providers(), 0);
        assert!(r.get_provider(&SubnetId::Tools, &p).is_none());
    }

    #[test]
    fn test_get_subnet_providers() {
        let mut r = initialized();
        let p1 = provider(0x04);
        let p2 = provider(0x05);
        r.register_provider(SubnetId::Storage, p1, isa(500), 1).unwrap();
        r.register_provider(SubnetId::Storage, p2, isa(600), 2).unwrap();

        let providers = r.get_subnet_providers(&SubnetId::Storage);
        assert_eq!(providers.len(), 2);
    }

    #[test]
    fn test_get_provider_subnets() {
        let mut r = initialized();
        let p = provider(0x06);
        r.register_provider(SubnetId::Model, p, isa(1_000), 1).unwrap();
        r.register_provider(SubnetId::Agent, p, isa(1_000), 1).unwrap();

        let mut subnets = r.get_provider_subnets(&p);
        subnets.sort_by_key(|s| format!("{:?}", s));
        assert_eq!(subnets.len(), 2);
        assert!(subnets.contains(&SubnetId::Model));
        assert!(subnets.contains(&SubnetId::Agent));
    }

    #[test]
    fn test_update_quality_score() {
        let mut r = initialized();
        let p = provider(0x07);
        r.register_provider(SubnetId::Compute, p, isa(2_000), 1).unwrap();

        r.update_quality_score(&SubnetId::Compute, &p, 8500).unwrap();
        assert_eq!(
            r.get_provider(&SubnetId::Compute, &p).unwrap().quality_score,
            8500
        );

        // Score is capped at 10 000.
        r.update_quality_score(&SubnetId::Compute, &p, 99_999).unwrap();
        assert_eq!(
            r.get_provider(&SubnetId::Compute, &p).unwrap().quality_score,
            10_000
        );
    }

    #[test]
    fn test_pause_resume_subnet() {
        let mut r = initialized();
        r.pause_subnet(&SubnetId::Tools, &admin()).unwrap();
        assert_eq!(
            r.get_subnet(&SubnetId::Tools).unwrap().status,
            SubnetStatus::Paused
        );

        r.resume_subnet(&SubnetId::Tools, &admin()).unwrap();
        assert_eq!(
            r.get_subnet(&SubnetId::Tools).unwrap().status,
            SubnetStatus::Active
        );
    }

    #[test]
    fn test_register_in_paused_subnet_fails() {
        let mut r = initialized();
        r.pause_subnet(&SubnetId::Tools, &admin()).unwrap();
        let err = r
            .register_provider(SubnetId::Tools, provider(0x08), isa(500), 10)
            .unwrap_err();
        assert_eq!(err, SubnetError::SubnetNotActive);
    }

    #[test]
    fn test_unauthorized_admin_fails() {
        let mut r = initialized();
        let bad = other_admin();

        let err = r.pause_subnet(&SubnetId::Model, &bad).unwrap_err();
        assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

        let err = r.resume_subnet(&SubnetId::Model, &bad).unwrap_err();
        assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

        let err = r
            .set_emission_weight(&SubnetId::Model, 1000, &bad)
            .unwrap_err();
        assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));

        let dummy_config = SubnetConfig {
            id: SubnetId::Market,
            name: "x".to_string(),
            description: "x".to_string(),
            min_provider_stake: isa(1),
            emission_weight: 500,
            fee_rate_bps: 100,
            max_providers: 10,
            status: SubnetStatus::Active,
            created_at: 0,
        };
        // Use a fresh registry (Market not yet registered) to reach the auth check.
        let mut r2 = SubnetRegistry::new(admin());
        let err = r2.register_subnet(dummy_config, &bad).unwrap_err();
        assert_eq!(err, SubnetError::UnauthorizedAdmin(bad));
    }

    #[test]
    fn test_duplicate_provider_fails() {
        let mut r = initialized();
        let p = provider(0x09);
        r.register_provider(SubnetId::Agent, p, isa(1_000), 1).unwrap();
        let err = r
            .register_provider(SubnetId::Agent, p, isa(1_000), 2)
            .unwrap_err();
        assert_eq!(err, SubnetError::ProviderAlreadyRegistered);
    }
}
