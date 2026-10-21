use super::types::{
    BridgeDirection, BridgeEventStatus, BridgeEventType, CreditBridgeError,
    CreditBridgeEvent,
};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

// ====================================================================
// CreditBridge
// ====================================================================

/// Bridges the off-chain `credit_service` (isA_user) with on-chain ISA Credits.
///
/// Tracks every cross-bridge operation (purchases, redemptions, grants, spends,
/// and transfers) and exposes aggregate statistics for the bridge.
///
/// ## Purchase flow (on-chain → off-chain)
/// 1. Call [`initiate_purchase`](Self::initiate_purchase) — burns ISA on-chain and queues credit
///    issuance.
/// 2. Off-chain service confirms credit issuance → call [`complete_event`](Self::complete_event).
///
/// ## Redemption flow (off-chain → on-chain)
/// 1. Call [`initiate_redemption`](Self::initiate_redemption) — burns off-chain credits and queues
///    ISA minting.
/// 2. On-chain mint confirmed → call [`complete_event`](Self::complete_event).
pub struct CreditBridge {
    /// All bridge events in insertion order.
    pub events: Vec<CreditBridgeEvent>,
    /// Per-user index: maps an address to the indices into `events`.
    pub by_user: HashMap<Address, Vec<usize>>,
    /// Number of events in a non-terminal state (`Initiated` or `Confirmed`).
    pub pending_count: usize,
    /// Lifetime total off-chain credits bridged (across completed events).
    pub total_credits_bridged: Amount,
    /// Lifetime total on-chain ISA bridged (across completed events).
    pub total_isa_bridged: Amount,
}

impl CreditBridge {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    /// Create a new, empty `CreditBridge`.
    pub fn new() -> Self {
        CreditBridge {
            events: Vec::new(),
            by_user: HashMap::new(),
            pending_count: 0,
            total_credits_bridged: 0,
            total_isa_bridged: 0,
        }
    }

    // ----------------------------------------------------------------
    // Bridge Operations
    // ----------------------------------------------------------------

    /// Initiate an on-chain → off-chain credit purchase.
    ///
    /// Validates that both amounts are non-zero, derives an event ID, and
    /// records the event in `Initiated` state.
    ///
    /// # Errors
    /// - [`CreditBridgeError::InvalidAmount`] if either amount is zero.
    pub fn initiate_purchase(
        &mut self,
        user: Address,
        isa_amount: Amount,
        credit_amount: Amount,
        height: BlockHeight,
    ) -> Result<Hash, CreditBridgeError> {
        if isa_amount == 0 || credit_amount == 0 {
            return Err(CreditBridgeError::InvalidAmount);
        }

        self.initiate_event(
            BridgeEventType::CreditPurchase,
            user,
            credit_amount,
            isa_amount,
            BridgeDirection::OnChainToOffChain,
            height,
        )
    }

    /// Initiate an off-chain → on-chain credit redemption.
    ///
    /// Validates that both amounts are non-zero, derives an event ID, and
    /// records the event in `Initiated` state.
    ///
    /// # Errors
    /// - [`CreditBridgeError::InvalidAmount`] if either amount is zero.
    pub fn initiate_redemption(
        &mut self,
        user: Address,
        credit_amount: Amount,
        isa_amount: Amount,
        height: BlockHeight,
    ) -> Result<Hash, CreditBridgeError> {
        if credit_amount == 0 || isa_amount == 0 {
            return Err(CreditBridgeError::InvalidAmount);
        }

        self.initiate_event(
            BridgeEventType::CreditRedemption,
            user,
            credit_amount,
            isa_amount,
            BridgeDirection::OffChainToOnChain,
            height,
        )
    }

    /// Mark an event as `Completed` and update cumulative bridge statistics.
    ///
    /// # Errors
    /// - [`CreditBridgeError::EventNotFound`] if `id` does not match any event.
    /// - [`CreditBridgeError::AlreadyCompleted`] if the event is already in a terminal state.
    pub fn complete_event(&mut self, id: &Hash) -> Result<(), CreditBridgeError> {
        let (credit_amount, isa_amount) = {
            let event = self
                .events
                .iter_mut()
                .find(|e| &e.id == id)
                .ok_or(CreditBridgeError::EventNotFound)?;

            match event.status {
                BridgeEventStatus::Completed
                | BridgeEventStatus::Failed(_)
                | BridgeEventStatus::RolledBack => {
                    return Err(CreditBridgeError::AlreadyCompleted);
                }
                _ => {}
            }

            event.status = BridgeEventStatus::Completed;
            (event.credit_amount, event.isa_amount)
        };

        self.pending_count = self.pending_count.saturating_sub(1);
        self.total_credits_bridged = self.total_credits_bridged.saturating_add(credit_amount);
        self.total_isa_bridged = self.total_isa_bridged.saturating_add(isa_amount);

        Ok(())
    }

    /// Mark an event as failed. Decrements `pending_count` without updating totals.
    ///
    /// # Errors
    /// - [`CreditBridgeError::EventNotFound`] if `id` does not match any event.
    /// - [`CreditBridgeError::AlreadyCompleted`] if the event is already terminal.
    pub fn fail_event(&mut self, id: &Hash, reason: String) -> Result<(), CreditBridgeError> {
        let event = self
            .events
            .iter_mut()
            .find(|e| &e.id == id)
            .ok_or(CreditBridgeError::EventNotFound)?;

        match event.status {
            BridgeEventStatus::Completed
            | BridgeEventStatus::Failed(_)
            | BridgeEventStatus::RolledBack => {
                return Err(CreditBridgeError::AlreadyCompleted);
            }
            _ => {}
        }

        event.status = BridgeEventStatus::Failed(reason);
        self.pending_count = self.pending_count.saturating_sub(1);

        Ok(())
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Look up an event by its ID.
    pub fn get_event(&self, id: &Hash) -> Option<&CreditBridgeEvent> {
        self.events.iter().find(|e| &e.id == id)
    }

    /// Retrieve all events involving `user`, in chronological order.
    pub fn get_user_events(&self, user: &Address) -> Vec<&CreditBridgeEvent> {
        self.by_user
            .get(user)
            .map(|indices| indices.iter().map(|&i| &self.events[i]).collect())
            .unwrap_or_default()
    }

    /// Return `(total_credits_bridged, total_isa_bridged)`.
    pub fn get_stats(&self) -> (Amount, Amount) {
        (self.total_credits_bridged, self.total_isa_bridged)
    }

    /// Number of bridge events currently in flight.
    pub fn get_pending_count(&self) -> usize {
        self.pending_count
    }

    // ----------------------------------------------------------------
    // Internal helpers
    // ----------------------------------------------------------------

    fn initiate_event(
        &mut self,
        event_type: BridgeEventType,
        user: Address,
        credit_amount: Amount,
        isa_amount: Amount,
        direction: BridgeDirection,
        height: BlockHeight,
    ) -> Result<Hash, CreditBridgeError> {
        let seq = self.events.len() as u64;
        let id = Self::derive_event_id(user, credit_amount, height, seq);

        let event = CreditBridgeEvent {
            id,
            event_type,
            user,
            credit_amount,
            isa_amount,
            direction,
            height,
            status: BridgeEventStatus::Initiated,
        };

        let idx = self.events.len();
        self.events.push(event);
        self.by_user.entry(user).or_default().push(idx);
        self.pending_count += 1;

        Ok(id)
    }

    /// Deterministic event ID from user + credit_amount + height + sequence.
    fn derive_event_id(
        user: Address,
        credit_amount: Amount,
        height: BlockHeight,
        seq: u64,
    ) -> Hash {
        let mut data = Vec::with_capacity(20 + 16 + 8 + 8);
        data.extend_from_slice(user.as_bytes());
        data.extend_from_slice(&credit_amount.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        data.extend_from_slice(&seq.to_le_bytes());
        Hash::hash_data(&data)
    }
}

impl Default for CreditBridge {
    fn default() -> Self {
        Self::new()
    }
}
