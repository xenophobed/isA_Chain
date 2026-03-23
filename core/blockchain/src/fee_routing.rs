use crate::types::{Address, Amount, BlockHeight};
use crate::subnet::SubnetId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// FeeRoute
// ============================================================================

/// A record of a single fee routing event for a subnet operation.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FeeRoute {
    /// The subnet this fee was routed from.
    pub subnet_id: SubnetId,
    /// Total gross amount received.
    pub gross_amount: Amount,
    /// Protocol cut — forwarded to ProtocolTreasury.
    pub protocol_fee: Amount,
    /// Subnet pool cut — stays in the subnet's fee pool.
    pub subnet_fee: Amount,
    /// Remainder — returned to the service provider.
    pub provider_amount: Amount,
    /// Block at which the routing occurred.
    pub height: BlockHeight,
}

// ============================================================================
// SubnetFeePool
// ============================================================================

/// Per-subnet pool that accumulates the subnet cut of every routed fee.
#[derive(Clone, Debug)]
pub struct SubnetFeePool {
    pub subnet_id: SubnetId,
    /// Current spendable balance in the pool.
    pub balance: Amount,
    /// Lifetime fees deposited into this pool.
    pub total_collected: Amount,
    /// Lifetime amount distributed out of this pool.
    pub total_distributed: Amount,
}

// ============================================================================
// FeeRoutingError
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FeeRoutingError {
    #[error("Subnet not found")]
    SubnetNotFound,

    #[error("Amount must be greater than zero")]
    InvalidAmount,

    #[error("Insufficient pool balance")]
    InsufficientPool,

    #[error("Invalid fee configuration: combined fee bps exceed 10000")]
    InvalidFeeConfig,
}

// ============================================================================
// FeeRouter
// ============================================================================

/// Routes fees from subnet operations to the protocol treasury, the subnet
/// pool, and the service provider.
pub struct FeeRouter {
    /// Per-subnet fee pools.
    pub pools: HashMap<SubnetId, SubnetFeePool>,
    /// Historical log of every fee routing event.
    pub routes: Vec<FeeRoute>,
    /// Protocol cut in basis points (default 250 = 2.5 %).
    pub protocol_fee_bps: u32,
    /// Subnet pool cut in basis points (default 250 = 2.5 %).
    pub subnet_fee_bps: u32,
    /// Lifetime gross amount routed through this router.
    pub total_routed: Amount,
}

impl FeeRouter {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new router.  Returns `FeeRoutingError::InvalidFeeConfig` if
    /// `protocol_fee_bps + subnet_fee_bps > 10 000`.
    pub fn new(protocol_fee_bps: u32, subnet_fee_bps: u32) -> Self {
        FeeRouter {
            pools: HashMap::new(),
            routes: Vec::new(),
            protocol_fee_bps,
            subnet_fee_bps,
            total_routed: 0,
        }
    }

    // ----------------------------------------------------------------
    // Pool management
    // ----------------------------------------------------------------

    /// Pre-create empty fee pools for each subnet in the slice.
    /// Silently skips subnets that already have a pool.
    pub fn initialize_pools(&mut self, subnets: &[SubnetId]) {
        for &subnet_id in subnets {
            self.pools.entry(subnet_id).or_insert(SubnetFeePool {
                subnet_id,
                balance: 0,
                total_collected: 0,
                total_distributed: 0,
            });
        }
    }

    // ----------------------------------------------------------------
    // Core routing
    // ----------------------------------------------------------------

    /// Split `gross_amount` across protocol treasury, subnet pool, and
    /// provider, record the route, and update the subnet pool balance.
    ///
    /// Fee split:
    /// ```text
    /// protocol_fee    = gross * protocol_fee_bps / 10_000
    /// subnet_fee      = gross * subnet_fee_bps   / 10_000
    /// provider_amount = gross - protocol_fee - subnet_fee
    /// ```
    ///
    /// Returns `FeeRoutingError::InvalidAmount` if `gross_amount` is zero.
    /// Returns `FeeRoutingError::SubnetNotFound` if no pool exists for the
    /// given subnet (call `initialize_pools` first).
    /// Returns `FeeRoutingError::InvalidFeeConfig` if the configured bps would
    /// exceed 10 000 in total.
    pub fn route_fee(
        &mut self,
        subnet_id: SubnetId,
        gross_amount: Amount,
        height: BlockHeight,
    ) -> Result<FeeRoute, FeeRoutingError> {
        if gross_amount == 0 {
            return Err(FeeRoutingError::InvalidAmount);
        }

        if self.protocol_fee_bps + self.subnet_fee_bps > 10_000 {
            return Err(FeeRoutingError::InvalidFeeConfig);
        }

        let pool = self
            .pools
            .get_mut(&subnet_id)
            .ok_or(FeeRoutingError::SubnetNotFound)?;

        let protocol_fee = gross_amount * self.protocol_fee_bps as Amount / 10_000;
        let subnet_fee = gross_amount * self.subnet_fee_bps as Amount / 10_000;
        let provider_amount = gross_amount - protocol_fee - subnet_fee;

        // Update the subnet pool.
        pool.balance += subnet_fee;
        pool.total_collected += subnet_fee;

        // Update lifetime total.
        self.total_routed += gross_amount;

        let route = FeeRoute {
            subnet_id,
            gross_amount,
            protocol_fee,
            subnet_fee,
            provider_amount,
            height,
        };

        self.routes.push(route.clone());
        Ok(route)
    }

    // ----------------------------------------------------------------
    // Distribution
    // ----------------------------------------------------------------

    /// Distribute funds from a subnet pool to a list of recipients.
    ///
    /// Each entry in `recipients` is `(address, amount)`.  The sum of all
    /// amounts must not exceed the pool's current balance.
    ///
    /// Returns the total amount distributed on success.
    pub fn distribute_pool(
        &mut self,
        subnet_id: &SubnetId,
        recipients: &[(Address, Amount)],
    ) -> Result<Amount, FeeRoutingError> {
        let total: Amount = recipients.iter().map(|(_, a)| a).sum();

        let pool = self
            .pools
            .get_mut(subnet_id)
            .ok_or(FeeRoutingError::SubnetNotFound)?;

        if total > pool.balance {
            return Err(FeeRoutingError::InsufficientPool);
        }

        pool.balance -= total;
        pool.total_distributed += total;

        Ok(total)
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Return a reference to the pool for `subnet_id`, or `None` if it does
    /// not exist.
    pub fn get_pool(&self, subnet_id: &SubnetId) -> Option<&SubnetFeePool> {
        self.pools.get(subnet_id)
    }

    /// Return the current balance of a subnet pool (0 if the pool does not
    /// exist).
    pub fn get_pool_balance(&self, subnet_id: &SubnetId) -> Amount {
        self.pools
            .get(subnet_id)
            .map(|p| p.balance)
            .unwrap_or(0)
    }

    /// Lifetime gross amount routed through this router.
    pub fn get_total_routed(&self) -> Amount {
        self.total_routed
    }

    /// All routing events that occurred at exactly `height`.
    pub fn get_routes_at_height(&self, height: BlockHeight) -> Vec<&FeeRoute> {
        self.routes.iter().filter(|r| r.height == height).collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn all_subnets() -> Vec<SubnetId> {
        vec![
            SubnetId::Model,
            SubnetId::Tools,
            SubnetId::Compute,
            SubnetId::Storage,
            SubnetId::Agent,
            SubnetId::Market,
        ]
    }

    fn default_router() -> FeeRouter {
        let mut router = FeeRouter::new(250, 250); // 2.5% + 2.5%
        router.initialize_pools(&all_subnets());
        router
    }

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    // ----------------------------------------------------------------

    #[test]
    fn test_route_fee() {
        let mut r = default_router();
        let route = r.route_fee(SubnetId::Model, 10_000, 1).unwrap();

        assert_eq!(route.subnet_id, SubnetId::Model);
        assert_eq!(route.gross_amount, 10_000);
        assert_eq!(route.protocol_fee, 250);  // 2.5%
        assert_eq!(route.subnet_fee, 250);    // 2.5%
        assert_eq!(route.provider_amount, 9_500); // remainder
        assert_eq!(route.height, 1);
    }

    #[test]
    fn test_fee_split_math() {
        let mut r = default_router();
        // 100_000 gross, 2.5% protocol, 2.5% subnet
        let route = r.route_fee(SubnetId::Tools, 100_000, 5).unwrap();

        assert_eq!(route.protocol_fee, 2_500);
        assert_eq!(route.subnet_fee, 2_500);
        assert_eq!(route.provider_amount, 95_000);
        // invariant: parts must sum to gross
        assert_eq!(
            route.protocol_fee + route.subnet_fee + route.provider_amount,
            route.gross_amount
        );
    }

    #[test]
    fn test_fee_split_math_asymmetric_rates() {
        // protocol 3%, subnet 1%
        let mut r = FeeRouter::new(300, 100);
        r.initialize_pools(&[SubnetId::Compute]);

        let route = r.route_fee(SubnetId::Compute, 10_000, 1).unwrap();
        assert_eq!(route.protocol_fee, 300);  // 3%
        assert_eq!(route.subnet_fee, 100);    // 1%
        assert_eq!(route.provider_amount, 9_600);
        assert_eq!(
            route.protocol_fee + route.subnet_fee + route.provider_amount,
            route.gross_amount
        );
    }

    #[test]
    fn test_initialize_pools() {
        let mut r = FeeRouter::new(250, 250);
        r.initialize_pools(&all_subnets());

        for id in all_subnets() {
            let pool = r.get_pool(&id).expect("pool should exist");
            assert_eq!(pool.balance, 0);
            assert_eq!(pool.total_collected, 0);
            assert_eq!(pool.total_distributed, 0);
        }
    }

    #[test]
    fn test_distribute_pool() {
        let mut r = default_router();
        r.route_fee(SubnetId::Storage, 100_000, 1).unwrap(); // subnet_fee = 2_500

        let recipients = vec![(addr(0x01), 1_000), (addr(0x02), 500)];
        let distributed = r.distribute_pool(&SubnetId::Storage, &recipients).unwrap();
        assert_eq!(distributed, 1_500);

        let pool = r.get_pool(&SubnetId::Storage).unwrap();
        assert_eq!(pool.balance, 2_500 - 1_500);          // 1_000 remaining
        assert_eq!(pool.total_distributed, 1_500);
    }

    #[test]
    fn test_distribute_insufficient() {
        let mut r = default_router();
        // Pool balance is 0 before any routing.
        let recipients = vec![(addr(0x01), 1_000)];
        let err = r
            .distribute_pool(&SubnetId::Agent, &recipients)
            .unwrap_err();
        assert_eq!(err, FeeRoutingError::InsufficientPool);
    }

    #[test]
    fn test_pool_balance_tracking() {
        let mut r = default_router();

        r.route_fee(SubnetId::Model, 10_000, 1).unwrap();  // +250
        r.route_fee(SubnetId::Model, 20_000, 2).unwrap();  // +500
        assert_eq!(r.get_pool_balance(&SubnetId::Model), 750);

        r.distribute_pool(&SubnetId::Model, &[(addr(0x01), 200)]).unwrap();
        assert_eq!(r.get_pool_balance(&SubnetId::Model), 550);
    }

    #[test]
    fn test_total_routed() {
        let mut r = default_router();
        assert_eq!(r.get_total_routed(), 0);

        r.route_fee(SubnetId::Model, 10_000, 1).unwrap();
        r.route_fee(SubnetId::Tools, 5_000, 2).unwrap();
        assert_eq!(r.get_total_routed(), 15_000);
    }

    #[test]
    fn test_routes_at_height() {
        let mut r = default_router();

        r.route_fee(SubnetId::Model, 10_000, 5).unwrap();
        r.route_fee(SubnetId::Tools, 5_000, 5).unwrap();
        r.route_fee(SubnetId::Compute, 3_000, 10).unwrap();

        let at_five = r.get_routes_at_height(5);
        assert_eq!(at_five.len(), 2);

        let at_ten = r.get_routes_at_height(10);
        assert_eq!(at_ten.len(), 1);
        assert_eq!(at_ten[0].subnet_id, SubnetId::Compute);

        let at_zero = r.get_routes_at_height(0);
        assert!(at_zero.is_empty());
    }

    #[test]
    fn test_zero_amount_fails() {
        let mut r = default_router();
        let err = r.route_fee(SubnetId::Model, 0, 1).unwrap_err();
        assert_eq!(err, FeeRoutingError::InvalidAmount);
    }

    #[test]
    fn test_multiple_subnets_independent() {
        let mut r = default_router();

        r.route_fee(SubnetId::Model, 40_000, 1).unwrap();   // subnet_fee = 1_000
        r.route_fee(SubnetId::Tools, 20_000, 1).unwrap();   // subnet_fee = 500
        r.route_fee(SubnetId::Storage, 10_000, 1).unwrap(); // subnet_fee = 250

        assert_eq!(r.get_pool_balance(&SubnetId::Model), 1_000);
        assert_eq!(r.get_pool_balance(&SubnetId::Tools), 500);
        assert_eq!(r.get_pool_balance(&SubnetId::Storage), 250);
        // Other subnets untouched
        assert_eq!(r.get_pool_balance(&SubnetId::Compute), 0);
        assert_eq!(r.get_pool_balance(&SubnetId::Agent), 0);
        assert_eq!(r.get_pool_balance(&SubnetId::Market), 0);
    }

    #[test]
    fn test_subnet_not_found() {
        // Router with no pools initialised.
        let mut r = FeeRouter::new(250, 250);
        let err = r.route_fee(SubnetId::Model, 1_000, 1).unwrap_err();
        assert_eq!(err, FeeRoutingError::SubnetNotFound);
    }

    #[test]
    fn test_invalid_fee_config() {
        // Combined bps > 10_000 should be rejected at routing time.
        let mut r = FeeRouter::new(6_000, 5_000);
        r.initialize_pools(&[SubnetId::Model]);
        let err = r.route_fee(SubnetId::Model, 1_000, 1).unwrap_err();
        assert_eq!(err, FeeRoutingError::InvalidFeeConfig);
    }
}
