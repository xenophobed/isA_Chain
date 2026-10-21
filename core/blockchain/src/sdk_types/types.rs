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
