use super::types::{BridgeError, ChainType, CrossChainTransfer, TransferStatus};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::{HashMap, HashSet};

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
