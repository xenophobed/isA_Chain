use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ============================================================================
// ChainType
// ============================================================================

/// Identifies a blockchain network participating in the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChainType {
    /// The native isA_Chain
    ISAChain,
    /// Ethereum mainnet (chain ID 1)
    Ethereum,
    /// Base L2 (chain ID 8453)
    Base,
    /// Any other chain identified by name
    Custom(String),
}

// ============================================================================
// TransferStatus
// ============================================================================

/// Lifecycle state of a cross-chain transfer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferStatus {
    /// Transfer submitted; not yet locked on source chain.
    Pending,
    /// ISA tokens locked in bridge escrow on source chain.
    Locked,
    /// Bridge relayer has submitted the mint on the destination chain.
    Minting,
    /// Mint confirmed on destination chain; transfer complete.
    Completed,
    /// Transfer failed; reason is included.
    Failed(String),
    /// Transfer was refunded back to the original sender.
    Refunded,
}

// ============================================================================
// CrossChainTransfer
// ============================================================================

/// An individual cross-chain transfer record.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrossChainTransfer {
    /// Unique transfer identifier (Blake3 hash of sender + amount + height).
    pub id: Hash,
    /// Source blockchain.
    pub source_chain: ChainType,
    /// Destination blockchain.
    pub dest_chain: ChainType,
    /// Sender's ISA chain address.
    pub sender: Address,
    /// Destination address on the external chain (0x-prefixed hex string).
    pub recipient_external: String,
    /// Gross amount of ISA tokens being transferred.
    pub amount: Amount,
    /// Bridge fee deducted from `amount` before minting on destination.
    pub fee: Amount,
    /// Current transfer lifecycle status.
    pub status: TransferStatus,
    /// Block height on isA_Chain when the transfer was created.
    pub created_at: BlockHeight,
    /// Block height when the transfer reached a terminal state.
    pub completed_at: Option<BlockHeight>,
    /// Transaction hash on the destination/external chain (set by relayer).
    pub external_tx_hash: Option<String>,
}

// ============================================================================
// BridgeError
// ============================================================================

/// Errors that can occur during bridge operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum BridgeError {
    #[error("Chain type is not supported by this bridge")]
    UnsupportedChain,

    #[error("Transfer amount is below the minimum required")]
    InsufficientAmount,

    #[error("Transfer amount exceeds the bridge maximum")]
    ExceedsMaxTransfer,

    #[error("Transfer not found: {0:?}")]
    TransferNotFound(Hash),

    #[error("Address is not an authorized relayer: {0:?}")]
    UnauthorizedRelayer(Address),

    #[error("Transfer has already reached a terminal state")]
    AlreadyCompleted,

    #[error("Recipient address is invalid (must be non-empty 0x-prefixed hex)")]
    InvalidAddress,

    #[error("Only the admin may perform this operation")]
    UnauthorizedAdmin,
}

// ============================================================================
// CrossChainBridge
// ============================================================================

/// Core bridge state managing ISA ↔ EVM transfers.
///
/// On lock (`initiate_transfer`): ISA tokens are conceptually removed from
/// the sender's balance on isA_Chain and credited to `total_locked`.
///
/// On mint confirmation (`confirm_transfer`): the relayer records the external
/// chain TX hash, moves ISA from `total_locked` to `total_transferred`, and
/// marks the transfer `Completed`.
///
/// On refund (`refund_transfer`): the relayer (or admin) returns the locked
/// amount minus fee back to the sender.
pub struct CrossChainBridge {
    /// All transfers indexed by ID.
    pub transfers: HashMap<Hash, CrossChainTransfer>,
    /// Index: sender address → list of transfer IDs.
    pub by_sender: HashMap<Address, Vec<Hash>>,
    /// EVM (or custom) chains the bridge is authorised to serve.
    pub supported_chains: HashSet<ChainType>,
    /// Bridge fee in basis points (100 bps = 1 %).
    pub fee_rate_bps: u32,
    /// Minimum transfer size (gross, before fee deduction).
    pub min_transfer: Amount,
    /// Maximum transfer size (gross).
    pub max_transfer: Amount,
    /// Total ISA currently locked in the bridge (pending or minting).
    pub total_locked: Amount,
    /// Cumulative ISA that has successfully crossed to external chains.
    pub total_transferred: Amount,
    /// Admin address — authorises relayers and updates configuration.
    pub admin: Address,
    /// Addresses allowed to confirm / refund transfers.
    pub relayers: HashSet<Address>,
}

impl CrossChainBridge {
    // -----------------------------------------------------------------------
    // Constructor
    // -----------------------------------------------------------------------

    /// Create a new bridge instance.
    ///
    /// `fee_bps` — fee in basis points (e.g. 100 = 1 %).
    /// `min_transfer` / `max_transfer` — gross amount bounds (inclusive).
    /// `admin` — the only address that can authorise relayers.
    pub fn new(
        fee_bps: u32,
        min_transfer: Amount,
        max_transfer: Amount,
        admin: Address,
    ) -> Self {
        // isA_Chain is always supported as the source side.
        let mut supported_chains = HashSet::new();
        supported_chains.insert(ChainType::ISAChain);

        CrossChainBridge {
            transfers: HashMap::new(),
            by_sender: HashMap::new(),
            supported_chains,
            fee_rate_bps: fee_bps,
            min_transfer,
            max_transfer,
            total_locked: 0,
            total_transferred: 0,
            admin,
            relayers: HashSet::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Configuration
    // -----------------------------------------------------------------------

    /// Register an additional chain so transfers targeting it are accepted.
    pub fn add_supported_chain(&mut self, chain: ChainType) {
        self.supported_chains.insert(chain);
    }

    /// Authorise an address to act as a bridge relayer.
    ///
    /// Only the `admin` may call this.
    pub fn add_relayer(&mut self, address: Address, admin: &Address) -> Result<(), BridgeError> {
        if *admin != self.admin {
            return Err(BridgeError::UnauthorizedAdmin);
        }
        self.relayers.insert(address);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Transfer lifecycle
    // -----------------------------------------------------------------------

    /// Lock ISA tokens and create a pending cross-chain transfer.
    ///
    /// Validates:
    /// - `dest_chain` is in `supported_chains`
    /// - `recipient` is non-empty and starts with `0x`
    /// - `amount` is within `[min_transfer, max_transfer]`
    ///
    /// Returns the unique transfer ID on success.
    pub fn initiate_transfer(
        &mut self,
        sender: Address,
        recipient: String,
        amount: Amount,
        dest_chain: ChainType,
        height: BlockHeight,
    ) -> Result<Hash, BridgeError> {
        // Validate destination chain.
        if !self.supported_chains.contains(&dest_chain) {
            return Err(BridgeError::UnsupportedChain);
        }

        // Validate recipient address format.
        if recipient.is_empty() || !recipient.starts_with("0x") {
            return Err(BridgeError::InvalidAddress);
        }

        // Validate amount bounds.
        if amount < self.min_transfer {
            return Err(BridgeError::InsufficientAmount);
        }
        if amount > self.max_transfer {
            return Err(BridgeError::ExceedsMaxTransfer);
        }

        // Calculate fee.
        let fee = self.calculate_fee(amount);

        // Derive a deterministic transfer ID from sender ++ amount ++ height ++ chain.
        let mut id_input = Vec::with_capacity(20 + 16 + 8 + 32);
        id_input.extend_from_slice(sender.as_bytes());
        id_input.extend_from_slice(&amount.to_le_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        // Include number of existing transfers for uniqueness when same params repeat.
        let seq = self.transfers.len() as u64;
        id_input.extend_from_slice(&seq.to_le_bytes());
        let id = Hash::hash_data(&id_input);

        let transfer = CrossChainTransfer {
            id,
            source_chain: ChainType::ISAChain,
            dest_chain,
            sender,
            recipient_external: recipient,
            amount,
            fee,
            status: TransferStatus::Locked,
            created_at: height,
            completed_at: None,
            external_tx_hash: None,
        };

        self.total_locked = self.total_locked.saturating_add(amount);
        self.by_sender.entry(sender).or_default().push(id);
        self.transfers.insert(id, transfer);

        Ok(id)
    }

    /// Record that the relayer has confirmed the mint on the external chain.
    ///
    /// Moves the transfer to `Completed`, debits `total_locked`, and credits
    /// `total_transferred` (net of fee).
    pub fn confirm_transfer(
        &mut self,
        id: &Hash,
        external_tx_hash: String,
        relayer: &Address,
        height: BlockHeight,
    ) -> Result<(), BridgeError> {
        if !self.relayers.contains(relayer) {
            return Err(BridgeError::UnauthorizedRelayer(*relayer));
        }

        let transfer = self
            .transfers
            .get_mut(id)
            .ok_or(BridgeError::TransferNotFound(*id))?;

        match transfer.status {
            TransferStatus::Locked | TransferStatus::Minting => {}
            _ => return Err(BridgeError::AlreadyCompleted),
        }

        let gross = transfer.amount;
        let fee = transfer.fee;
        let net = gross.saturating_sub(fee);

        transfer.status = TransferStatus::Completed;
        transfer.completed_at = Some(height);
        transfer.external_tx_hash = Some(external_tx_hash);

        self.total_locked = self.total_locked.saturating_sub(gross);
        self.total_transferred = self.total_transferred.saturating_add(net);

        Ok(())
    }

    /// Refund a locked transfer back to the original sender.
    ///
    /// Returns the net refund amount (gross minus fee).
    /// The caller is responsible for actually crediting the sender's balance.
    pub fn refund_transfer(
        &mut self,
        id: &Hash,
        relayer: &Address,
    ) -> Result<Amount, BridgeError> {
        if !self.relayers.contains(relayer) {
            return Err(BridgeError::UnauthorizedRelayer(*relayer));
        }

        let transfer = self
            .transfers
            .get_mut(id)
            .ok_or(BridgeError::TransferNotFound(*id))?;

        match transfer.status {
            TransferStatus::Locked | TransferStatus::Minting | TransferStatus::Pending => {}
            _ => return Err(BridgeError::AlreadyCompleted),
        }

        let gross = transfer.amount;
        let fee = transfer.fee;
        let refund = gross.saturating_sub(fee);

        transfer.status = TransferStatus::Refunded;
        self.total_locked = self.total_locked.saturating_sub(gross);

        Ok(refund)
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Look up a single transfer by ID.
    pub fn get_transfer(&self, id: &Hash) -> Option<&CrossChainTransfer> {
        self.transfers.get(id)
    }

    /// Return all transfers initiated by `sender`, in insertion order.
    pub fn get_sender_transfers(&self, sender: &Address) -> Vec<&CrossChainTransfer> {
        self.by_sender
            .get(sender)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.transfers.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Total ISA currently locked in the bridge (pending + minting).
    pub fn get_total_locked(&self) -> Amount {
        self.total_locked
    }

    /// Calculate the bridge fee for a given gross `amount`.
    ///
    /// `fee = amount * fee_rate_bps / 10_000`  (rounded down).
    pub fn calculate_fee(&self, amount: Amount) -> Amount {
        amount
            .saturating_mul(self.fee_rate_bps as u128)
            / 10_000
    }

    /// List all currently supported destination chains.
    pub fn get_supported_chains(&self) -> Vec<&ChainType> {
        self.supported_chains.iter().collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ---- helpers -----------------------------------------------------------

    fn admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn relayer() -> Address {
        Address::from([0xBB; 20])
    }

    fn sender() -> Address {
        Address::from([0xCC; 20])
    }

    fn stranger() -> Address {
        Address::from([0xDD; 20])
    }

    /// Default bridge: 1 % fee, 100–1 000 000 ISA range, Ethereum + Base supported.
    fn setup() -> CrossChainBridge {
        let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
        bridge.add_supported_chain(ChainType::Ethereum);
        bridge.add_supported_chain(ChainType::Base);
        bridge.add_relayer(relayer(), &admin()).unwrap();
        bridge
    }

    fn valid_recipient() -> String {
        "0xAbCdEf1234567890AbCdEf1234567890AbCdEf12".to_string()
    }

    // ---- constructor -------------------------------------------------------

    #[test]
    fn test_new_bridge_defaults() {
        let bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
        assert_eq!(bridge.fee_rate_bps, 100);
        assert_eq!(bridge.min_transfer, 100);
        assert_eq!(bridge.max_transfer, 1_000_000);
        assert_eq!(bridge.total_locked, 0);
        assert_eq!(bridge.total_transferred, 0);
        assert!(bridge.supported_chains.contains(&ChainType::ISAChain));
    }

    // ---- chain support -----------------------------------------------------

    #[test]
    fn test_add_supported_chain() {
        let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
        bridge.add_supported_chain(ChainType::Ethereum);
        assert!(bridge.supported_chains.contains(&ChainType::Ethereum));
    }

    #[test]
    fn test_custom_chain_supported() {
        let mut bridge = CrossChainBridge::new(100, 100, 1_000_000, admin());
        bridge.add_supported_chain(ChainType::Custom("Polygon".to_string()));
        assert!(bridge
            .supported_chains
            .contains(&ChainType::Custom("Polygon".to_string())));
    }

    // ---- relayer management ------------------------------------------------

    #[test]
    fn test_add_relayer_as_admin() {
        let bridge = setup();
        assert!(bridge.relayers.contains(&relayer()));
    }

    #[test]
    fn test_add_relayer_non_admin_fails() {
        let mut bridge = setup();
        let result = bridge.add_relayer(stranger(), &stranger());
        assert_eq!(result, Err(BridgeError::UnauthorizedAdmin));
    }

    // ---- initiate_transfer -------------------------------------------------

    #[test]
    fn test_initiate_transfer_success() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 42)
            .unwrap();

        let t = bridge.get_transfer(&id).unwrap();
        assert_eq!(t.sender, sender());
        assert_eq!(t.amount, 1_000);
        assert_eq!(t.dest_chain, ChainType::Ethereum);
        assert_eq!(t.status, TransferStatus::Locked);
        assert_eq!(bridge.get_total_locked(), 1_000);
    }

    #[test]
    fn test_initiate_transfer_unsupported_chain() {
        let mut bridge = setup();
        let result = bridge.initiate_transfer(
            sender(),
            valid_recipient(),
            1_000,
            ChainType::Custom("Unknown".to_string()),
            1,
        );
        assert_eq!(result, Err(BridgeError::UnsupportedChain));
    }

    #[test]
    fn test_initiate_transfer_below_minimum() {
        let mut bridge = setup();
        let result = bridge.initiate_transfer(
            sender(),
            valid_recipient(),
            50, // < min_transfer (100)
            ChainType::Ethereum,
            1,
        );
        assert_eq!(result, Err(BridgeError::InsufficientAmount));
    }

    #[test]
    fn test_initiate_transfer_exceeds_maximum() {
        let mut bridge = setup();
        let result = bridge.initiate_transfer(
            sender(),
            valid_recipient(),
            2_000_000, // > max_transfer (1_000_000)
            ChainType::Ethereum,
            1,
        );
        assert_eq!(result, Err(BridgeError::ExceedsMaxTransfer));
    }

    #[test]
    fn test_initiate_transfer_invalid_recipient_empty() {
        let mut bridge = setup();
        let result = bridge.initiate_transfer(
            sender(),
            String::new(),
            1_000,
            ChainType::Ethereum,
            1,
        );
        assert_eq!(result, Err(BridgeError::InvalidAddress));
    }

    #[test]
    fn test_initiate_transfer_invalid_recipient_no_0x_prefix() {
        let mut bridge = setup();
        let result = bridge.initiate_transfer(
            sender(),
            "AbCdEf1234567890AbCdEf1234567890AbCdEf12".to_string(),
            1_000,
            ChainType::Ethereum,
            1,
        );
        assert_eq!(result, Err(BridgeError::InvalidAddress));
    }

    // ---- fee calculation ---------------------------------------------------

    #[test]
    fn test_calculate_fee_one_percent() {
        let bridge = setup(); // 100 bps = 1 %
        assert_eq!(bridge.calculate_fee(10_000), 100);
        assert_eq!(bridge.calculate_fee(1_000), 10);
        assert_eq!(bridge.calculate_fee(100), 1);
    }

    #[test]
    fn test_calculate_fee_rounds_down() {
        let bridge = setup(); // 1 %
        // 1 % of 199 = 1.99 → floors to 1
        assert_eq!(bridge.calculate_fee(199), 1);
    }

    // ---- confirm_transfer --------------------------------------------------

    #[test]
    fn test_confirm_transfer_success() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 10_000, ChainType::Base, 1)
            .unwrap();

        bridge
            .confirm_transfer(&id, "0xdeadbeef".to_string(), &relayer(), 10)
            .unwrap();

        let t = bridge.get_transfer(&id).unwrap();
        assert_eq!(t.status, TransferStatus::Completed);
        assert_eq!(t.external_tx_hash, Some("0xdeadbeef".to_string()));
        assert_eq!(t.completed_at, Some(10));
        // locked should be zero again, transferred = net
        assert_eq!(bridge.get_total_locked(), 0);
        assert_eq!(bridge.total_transferred, 10_000 - bridge.calculate_fee(10_000));
    }

    #[test]
    fn test_confirm_transfer_unauthorized_relayer() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
            .unwrap();

        let result =
            bridge.confirm_transfer(&id, "0xdeadbeef".to_string(), &stranger(), 5);
        assert_eq!(result, Err(BridgeError::UnauthorizedRelayer(stranger())));
    }

    #[test]
    fn test_confirm_transfer_not_found() {
        let mut bridge = setup();
        let fake_id = Hash::hash_data(b"nonexistent");
        let result =
            bridge.confirm_transfer(&fake_id, "0xdeadbeef".to_string(), &relayer(), 5);
        assert_eq!(result, Err(BridgeError::TransferNotFound(fake_id)));
    }

    #[test]
    fn test_confirm_transfer_already_completed() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
            .unwrap();

        bridge
            .confirm_transfer(&id, "0xabc".to_string(), &relayer(), 2)
            .unwrap();

        // Second confirm should fail.
        let result = bridge.confirm_transfer(&id, "0xabc".to_string(), &relayer(), 3);
        assert_eq!(result, Err(BridgeError::AlreadyCompleted));
    }

    // ---- refund_transfer ---------------------------------------------------

    #[test]
    fn test_refund_transfer_success() {
        let mut bridge = setup();
        let amount = 5_000u128;
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), amount, ChainType::Ethereum, 1)
            .unwrap();

        let fee = bridge.calculate_fee(amount);
        let refund = bridge.refund_transfer(&id, &relayer()).unwrap();

        assert_eq!(refund, amount - fee);
        let t = bridge.get_transfer(&id).unwrap();
        assert_eq!(t.status, TransferStatus::Refunded);
        assert_eq!(bridge.get_total_locked(), 0);
    }

    #[test]
    fn test_refund_transfer_unauthorized_relayer() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
            .unwrap();

        let result = bridge.refund_transfer(&id, &stranger());
        assert_eq!(result, Err(BridgeError::UnauthorizedRelayer(stranger())));
    }

    #[test]
    fn test_refund_after_complete_fails() {
        let mut bridge = setup();
        let id = bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
            .unwrap();

        bridge
            .confirm_transfer(&id, "0xabc".to_string(), &relayer(), 2)
            .unwrap();

        let result = bridge.refund_transfer(&id, &relayer());
        assert_eq!(result, Err(BridgeError::AlreadyCompleted));
    }

    // ---- get_sender_transfers ----------------------------------------------

    #[test]
    fn test_get_sender_transfers() {
        let mut bridge = setup();

        bridge
            .initiate_transfer(sender(), valid_recipient(), 500, ChainType::Ethereum, 1)
            .unwrap();
        bridge
            .initiate_transfer(sender(), valid_recipient(), 600, ChainType::Base, 2)
            .unwrap();

        let transfers = bridge.get_sender_transfers(&sender());
        assert_eq!(transfers.len(), 2);
    }

    #[test]
    fn test_get_sender_transfers_empty_for_unknown() {
        let bridge = setup();
        let transfers = bridge.get_sender_transfers(&stranger());
        assert!(transfers.is_empty());
    }

    // ---- get_supported_chains ----------------------------------------------

    #[test]
    fn test_get_supported_chains_contains_added() {
        let bridge = setup();
        let chains = bridge.get_supported_chains();
        let chain_list: Vec<_> = chains.iter().map(|c| (*c).clone()).collect();
        assert!(chain_list.contains(&ChainType::Ethereum));
        assert!(chain_list.contains(&ChainType::Base));
        assert!(chain_list.contains(&ChainType::ISAChain));
    }

    // ---- total_locked accounting -------------------------------------------

    #[test]
    fn test_total_locked_accumulates_across_transfers() {
        let mut bridge = setup();

        bridge
            .initiate_transfer(sender(), valid_recipient(), 1_000, ChainType::Ethereum, 1)
            .unwrap();
        bridge
            .initiate_transfer(sender(), valid_recipient(), 2_000, ChainType::Base, 2)
            .unwrap();

        assert_eq!(bridge.get_total_locked(), 3_000);
    }
}
