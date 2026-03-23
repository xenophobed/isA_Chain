//! SDK Types — Unified Interface for External Service Integration
//!
//! This module exposes clean types and interfaces for external services
//! (isA_App_SDK, isA_Agent_SDK, isA_Mate, isA_Trade) to interact with
//! isA_Chain. Covers issues #22, #44, #46, #48, #51, #53, #57.

use crate::agent_registry::{AgentCapability, AgentPricing};
use crate::settlement::ServiceType;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, ChainId, Hash, Timestamp};
use serde::{Deserialize, Serialize};

// ============================================================================
// SdkVersion (#all issues — version negotiation)
// ============================================================================

/// Version descriptor for the isA_Chain SDK interface.
///
/// Callers should check `is_compatible` before making chain calls to ensure
/// their compiled expectations match the running chain version.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SdkVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    pub chain_id: ChainId,
}

impl SdkVersion {
    /// Current SDK version shipped with this crate (0.1.0 on chain 1).
    pub fn current() -> Self {
        SdkVersion {
            major: 0,
            minor: 1,
            patch: 0,
            chain_id: 1,
        }
    }

    /// Two versions are compatible when they share the same major version
    /// and chain ID.
    pub fn is_compatible(&self, other: &Self) -> bool {
        self.major == other.major && self.chain_id == other.chain_id
    }
}

// ============================================================================
// WalletEndpoint (#22 — wallet query surface for isA_App_SDK)
// ============================================================================

/// Describes every read-only query an external SDK can make against a wallet.
///
/// Variants map 1-to-1 with RPC handler arms so call-sites remain strongly typed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WalletEndpoint {
    /// Native ISA / gas balance for the address
    GetBalance(Address),
    /// Current transaction nonce (used for replay protection)
    GetNonce(Address),
    /// Full account record (balance + nonce + metadata)
    GetAccount(Address),
    /// Paginated transaction history; second field is the page limit
    GetTransactionHistory(Address, usize),
    /// ISA token balance (ERC-20-style)
    GetTokenBalance(Address),
    /// Prepaid credit balance in micro-credits
    GetCreditBalance(Address),
    /// Active stake amount and unlock epoch
    GetStakeInfo(Address),
}

// ============================================================================
// PaymentMethod / CreditTopUpRequest (#44 — credit top-up flow)
// ============================================================================

/// How a user intends to fund a credit top-up.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentMethod {
    /// Burn ISA tokens directly from the user's on-chain wallet
    OnChainISA,
    /// Off-ramp via credit card (handled by fiat bridge)
    CreditCard,
    /// Bank transfer / ACH / SEPA (handled by fiat bridge)
    BankTransfer,
    /// Any supported third-party crypto exchange
    CryptoExchange,
}

/// A request to top up a user's credit balance.
///
/// The chain-side handler validates `isa_amount` against the current
/// ISA/credit exchange rate and mints credits accordingly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditTopUpRequest {
    /// The wallet address receiving the credits
    pub user: Address,
    /// Amount of ISA (in base units) to exchange for credits
    pub isa_amount: Amount,
    /// How the user is providing ISA
    pub payment_method: PaymentMethod,
    /// Unix timestamp (ms) when the request was created; used for TTL checks
    pub timestamp: Timestamp,
}

// ============================================================================
// ChannelManagerConfig / ChannelManagerAction (#46 — Agent SDK channel mgmt)
// ============================================================================

/// Runtime configuration for an SDK-managed payment channel pool.
///
/// isA_Agent_SDK reads these settings to decide when to open, top-up, or
/// close channels automatically without user intervention.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelManagerConfig {
    /// Open a new channel automatically when the pending payment exceeds this amount
    pub auto_open_threshold: Amount,
    /// Close a channel after it has been idle for this many blocks
    pub auto_close_idle_blocks: u64,
    /// Default on-chain deposit when opening a new channel
    pub default_deposit: Amount,
    /// Blocks both parties must wait during a unilateral close dispute window
    pub default_dispute_period: u64,
}

/// Actions the SDK channel manager can emit to the chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ChannelManagerAction {
    /// Open a new two-party payment channel
    OpenChannel {
        sender: Address,
        receiver: Address,
        deposit: Amount,
    },
    /// Cooperatively close a channel and settle balances
    CloseChannel { channel_id: Hash },
    /// Publish a new off-chain state update on-chain (dispute / checkpoint)
    UpdateState {
        channel_id: Hash,
        sender_balance: Amount,
        receiver_balance: Amount,
    },
    /// Add more funds to an already-open channel
    TopUp { channel_id: Hash, amount: Amount },
}

// ============================================================================
// ChannelHubConfig (#48 — channel hub for high-traffic routes)
// ============================================================================

/// Configuration for a hub node that aggregates channels for a set of
/// high-traffic service routes.
///
/// Hub nodes reduce on-chain footprint by routing many logical payment flows
/// through a smaller number of funded channels.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelHubConfig {
    /// The hub's own on-chain address (receives consolidated settlements)
    pub hub_address: Address,
    /// Ordered list of `(service, provider_address)` pairs this hub supports
    pub supported_routes: Vec<(ServiceType, Address)>,
    /// Maximum number of simultaneously open channels per route
    pub max_channels_per_route: usize,
    /// Perform bulk on-chain settlement every N blocks
    pub settlement_interval_blocks: u64,
}

// ============================================================================
// AgentDiscoveryQuery / AgentDiscoveryResult (#51 — agent discovery)
// ============================================================================

/// A structured query for discovering registered agents on-chain.
///
/// All fields are optional filters; an empty query returns up to `limit` agents
/// ordered by reputation descending.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentDiscoveryQuery {
    /// Only return agents that advertise all of these capabilities
    pub capabilities: Vec<AgentCapability>,
    /// Filter out agents whose reputation score is below this threshold
    pub min_reputation: Option<u32>,
    /// Filter out agents whose base fee exceeds this amount
    pub max_price: Option<Amount>,
    /// Maximum number of results to return
    pub limit: usize,
}

/// A single result entry returned by agent discovery.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AgentDiscoveryResult {
    /// Unique on-chain agent identifier
    pub agent_id: Hash,
    /// Human-readable agent name
    pub name: String,
    /// Capabilities this agent has registered
    pub capabilities: Vec<AgentCapability>,
    /// Aggregated reputation score (0–1000)
    pub reputation: u32,
    /// Fee structure for invoking this agent
    pub pricing: AgentPricing,
    /// The agent's payment wallet address
    pub wallet: Address,
}

// ============================================================================
// CompanionWalletConfig (#53 — isA_Mate integration)
// ============================================================================

/// Wallet configuration for a companion agent (isA_Mate).
///
/// Companion agents operate with a constrained budget carved out of the
/// owner's main wallet, with optional automatic staking of idle funds.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CompanionWalletConfig {
    /// The human owner's wallet address
    pub owner: Address,
    /// On-chain ID of the companion agent being configured
    pub companion_agent_id: Hash,
    /// Maximum amount the companion may spend per day (in base ISA units)
    pub daily_allowance: Amount,
    /// When true, idle balance is automatically staked to earn yield
    pub auto_stake: bool,
    /// Ordered list of subnets the companion should prefer for service calls
    pub preferred_subnets: Vec<SubnetId>,
}

// ============================================================================
// TradeAgentConfig (#57 — isA_Trade integration)
// ============================================================================

/// On-chain configuration for an autonomous trading agent.
///
/// The chain uses this record to enforce position limits and route settlement
/// proceeds back to the correct wallet.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeAgentConfig {
    /// On-chain ID of the trading agent
    pub agent_id: Hash,
    /// Wallet that funds positions and receives profits
    pub wallet: Address,
    /// Trading pairs the agent is authorised to trade, e.g. `[("ISA", "USDC")]`
    pub trading_pairs: Vec<(String, String)>,
    /// Maximum notional size per position (in base ISA units)
    pub max_position_size: Amount,
    /// Stop-loss threshold in basis points (e.g. 200 = 2%)
    pub stop_loss_bps: u32,
    /// Take-profit threshold in basis points (e.g. 500 = 5%)
    pub take_profit_bps: u32,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Hash;

    fn zero_addr() -> Address {
        Address::new([0u8; 20])
    }

    fn zero_hash() -> Hash {
        Hash::ZERO
    }

    // -------------------------------------------------------------------------
    // #22 — WalletEndpoint
    // -------------------------------------------------------------------------

    #[test]
    fn test_wallet_endpoints() {
        let addr = zero_addr();
        let endpoints = vec![
            WalletEndpoint::GetBalance(addr),
            WalletEndpoint::GetNonce(addr),
            WalletEndpoint::GetAccount(addr),
            WalletEndpoint::GetTransactionHistory(addr, 50),
            WalletEndpoint::GetTokenBalance(addr),
            WalletEndpoint::GetCreditBalance(addr),
            WalletEndpoint::GetStakeInfo(addr),
        ];
        // All variants must be constructable and cloneable
        for ep in &endpoints {
            let _ = ep.clone();
        }
        assert_eq!(endpoints.len(), 7);
    }

    #[test]
    fn test_wallet_endpoint_history_limit() {
        let ep = WalletEndpoint::GetTransactionHistory(zero_addr(), 100);
        if let WalletEndpoint::GetTransactionHistory(_, limit) = ep {
            assert_eq!(limit, 100);
        } else {
            panic!("wrong variant");
        }
    }

    // -------------------------------------------------------------------------
    // #44 — CreditTopUpRequest / PaymentMethod
    // -------------------------------------------------------------------------

    #[test]
    fn test_payment_methods() {
        let methods = vec![
            PaymentMethod::OnChainISA,
            PaymentMethod::CreditCard,
            PaymentMethod::BankTransfer,
            PaymentMethod::CryptoExchange,
        ];
        for m in &methods {
            let cloned = m.clone();
            assert_eq!(*m, cloned);
        }
    }

    #[test]
    fn test_credit_top_up_request() {
        let req = CreditTopUpRequest {
            user: zero_addr(),
            isa_amount: 1_000_000,
            payment_method: PaymentMethod::OnChainISA,
            timestamp: 1_700_000_000_000,
        };
        assert_eq!(req.isa_amount, 1_000_000);
        assert_eq!(req.payment_method, PaymentMethod::OnChainISA);
    }

    // -------------------------------------------------------------------------
    // #46 — ChannelManagerConfig / ChannelManagerAction
    // -------------------------------------------------------------------------

    #[test]
    fn test_channel_manager_config() {
        let cfg = ChannelManagerConfig {
            auto_open_threshold: 500,
            auto_close_idle_blocks: 1_000,
            default_deposit: 10_000,
            default_dispute_period: 50,
        };
        assert_eq!(cfg.auto_open_threshold, 500);
        assert_eq!(cfg.auto_close_idle_blocks, 1_000);
        assert_eq!(cfg.default_deposit, 10_000);
        assert_eq!(cfg.default_dispute_period, 50);
    }

    #[test]
    fn test_channel_actions() {
        let addr = zero_addr();
        let hash = zero_hash();

        let open = ChannelManagerAction::OpenChannel {
            sender: addr,
            receiver: addr,
            deposit: 10_000,
        };
        let close = ChannelManagerAction::CloseChannel { channel_id: hash };
        let update = ChannelManagerAction::UpdateState {
            channel_id: hash,
            sender_balance: 6_000,
            receiver_balance: 4_000,
        };
        let topup = ChannelManagerAction::TopUp {
            channel_id: hash,
            amount: 5_000,
        };

        // Verify all variants are reachable
        match open {
            ChannelManagerAction::OpenChannel { deposit, .. } => assert_eq!(deposit, 10_000),
            _ => panic!("wrong variant"),
        }
        match close {
            ChannelManagerAction::CloseChannel { .. } => {}
            _ => panic!("wrong variant"),
        }
        match update {
            ChannelManagerAction::UpdateState {
                sender_balance,
                receiver_balance,
                ..
            } => {
                assert_eq!(sender_balance + receiver_balance, 10_000);
            }
            _ => panic!("wrong variant"),
        }
        match topup {
            ChannelManagerAction::TopUp { amount, .. } => assert_eq!(amount, 5_000),
            _ => panic!("wrong variant"),
        }
    }

    // -------------------------------------------------------------------------
    // #48 — ChannelHubConfig
    // -------------------------------------------------------------------------

    #[test]
    fn test_channel_hub_config() {
        let cfg = ChannelHubConfig {
            hub_address: zero_addr(),
            supported_routes: vec![
                (ServiceType::ModelInference, zero_addr()),
                (ServiceType::ToolExecution, zero_addr()),
            ],
            max_channels_per_route: 16,
            settlement_interval_blocks: 100,
        };
        assert_eq!(cfg.supported_routes.len(), 2);
        assert_eq!(cfg.max_channels_per_route, 16);
        assert_eq!(cfg.settlement_interval_blocks, 100);
    }

    #[test]
    fn test_service_type_custom() {
        let svc = ServiceType::Custom("video-transcoding".to_string());
        if let ServiceType::Custom(name) = svc {
            assert_eq!(name, "video-transcoding");
        } else {
            panic!("wrong variant");
        }
    }

    // -------------------------------------------------------------------------
    // #51 — AgentDiscoveryQuery / AgentDiscoveryResult
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // #53 — CompanionWalletConfig
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // #57 — TradeAgentConfig
    // -------------------------------------------------------------------------

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

    // -------------------------------------------------------------------------
    // SdkVersion
    // -------------------------------------------------------------------------

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
}
