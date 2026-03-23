use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ====================================================================
// BridgeEventType
// ====================================================================

/// The class of operation being bridged between off-chain credits and on-chain ISA.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeEventType {
    /// Buy credits with ISA — ISA burned on-chain, credits issued off-chain.
    CreditPurchase,
    /// Convert credits to ISA — credits burned off-chain, ISA minted on-chain.
    CreditRedemption,
    /// Move credits between accounts (purely off-chain).
    CreditTransfer,
    /// Admin-granted credits (no ISA movement).
    CreditGrant,
    /// Credits spent on a platform service.
    CreditSpend,
}

// ====================================================================
// BridgeDirection
// ====================================================================

/// Which direction value is flowing across the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeDirection {
    /// ISA is burned on-chain; credits are issued off-chain.
    OnChainToOffChain,
    /// Credits are burned off-chain; ISA is minted on-chain.
    OffChainToOnChain,
}

// ====================================================================
// BridgeEventStatus
// ====================================================================

/// Lifecycle state of a single bridge event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BridgeEventStatus {
    /// Bridge event created; awaiting confirmation from both sides.
    Initiated,
    /// On-chain or off-chain side has confirmed; waiting for the other.
    Confirmed,
    /// Both sides confirmed; bridge event is settled.
    Completed,
    /// Bridge event failed with an attached reason.
    Failed(String),
    /// Bridge event was rolled back after a failure.
    RolledBack,
}

// ====================================================================
// CreditBridgeEvent
// ====================================================================

/// A single bridging event between the off-chain credit_service and on-chain ISA.
///
/// ## Lifecycle
/// `Initiated` → `Confirmed` → `Completed`
///                          ↘ `Failed(reason)` → `RolledBack`
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreditBridgeEvent {
    /// Unique event identifier.
    pub id: Hash,
    /// The kind of bridge operation.
    pub event_type: BridgeEventType,
    /// The user account involved.
    pub user: Address,
    /// Off-chain credit units involved in this event.
    pub credit_amount: Amount,
    /// On-chain ISA units (micro-ISA) involved in this event.
    pub isa_amount: Amount,
    /// Direction of value flow.
    pub direction: BridgeDirection,
    /// Block height when the event was initiated.
    pub height: BlockHeight,
    /// Current lifecycle status.
    pub status: BridgeEventStatus,
}

// ====================================================================
// CreditBridgeError
// ====================================================================

/// Errors that can occur during credit bridge operations.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CreditBridgeError {
    #[error("Event not found")]
    EventNotFound,

    #[error("Invalid amount: must be greater than zero")]
    InvalidAmount,

    #[error("Insufficient credits to complete the bridge operation")]
    InsufficientCredits,

    #[error("Insufficient ISA to complete the bridge operation")]
    InsufficientISA,

    #[error("Bridge event has already been completed")]
    AlreadyCompleted,
}

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
    // Initiation helpers
    // ----------------------------------------------------------------

    /// Initiate an ISA-to-credits purchase (on-chain → off-chain).
    ///
    /// Records a new bridge event in `Initiated` state.
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

        let id = Self::derive_id(user, isa_amount, credit_amount, height);
        let event = CreditBridgeEvent {
            id,
            event_type: BridgeEventType::CreditPurchase,
            user,
            credit_amount,
            isa_amount,
            direction: BridgeDirection::OnChainToOffChain,
            height,
            status: BridgeEventStatus::Initiated,
        };

        self.insert_event(event);
        Ok(id)
    }

    /// Initiate a credits-to-ISA redemption (off-chain → on-chain).
    ///
    /// Records a new bridge event in `Initiated` state.
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

        let id = Self::derive_id(user, isa_amount, credit_amount, height);
        let event = CreditBridgeEvent {
            id,
            event_type: BridgeEventType::CreditRedemption,
            user,
            credit_amount,
            isa_amount,
            direction: BridgeDirection::OffChainToOnChain,
            height,
            status: BridgeEventStatus::Initiated,
        };

        self.insert_event(event);
        Ok(id)
    }

    // ----------------------------------------------------------------
    // Lifecycle transitions
    // ----------------------------------------------------------------

    /// Mark a bridge event as `Completed` and update aggregate stats.
    ///
    /// # Errors
    /// - [`CreditBridgeError::EventNotFound`] if `event_id` is unknown.
    /// - [`CreditBridgeError::AlreadyCompleted`] if the event is already in a terminal state.
    pub fn complete_event(&mut self, event_id: &Hash) -> Result<(), CreditBridgeError> {
        let idx = self
            .find_index(event_id)
            .ok_or(CreditBridgeError::EventNotFound)?;

        let event = &self.events[idx];
        match &event.status {
            BridgeEventStatus::Completed
            | BridgeEventStatus::Failed(_)
            | BridgeEventStatus::RolledBack => {
                return Err(CreditBridgeError::AlreadyCompleted);
            }
            _ => {}
        }

        let credits = event.credit_amount;
        let isa = event.isa_amount;

        self.events[idx].status = BridgeEventStatus::Completed;
        self.pending_count = self.pending_count.saturating_sub(1);
        self.total_credits_bridged = self.total_credits_bridged.saturating_add(credits);
        self.total_isa_bridged = self.total_isa_bridged.saturating_add(isa);

        Ok(())
    }

    /// Mark a bridge event as `Failed` with a reason.
    ///
    /// # Errors
    /// - [`CreditBridgeError::EventNotFound`] if `event_id` is unknown.
    /// - [`CreditBridgeError::AlreadyCompleted`] if the event is already in a terminal state.
    pub fn fail_event(
        &mut self,
        event_id: &Hash,
        reason: String,
    ) -> Result<(), CreditBridgeError> {
        let idx = self
            .find_index(event_id)
            .ok_or(CreditBridgeError::EventNotFound)?;

        match &self.events[idx].status {
            BridgeEventStatus::Completed
            | BridgeEventStatus::Failed(_)
            | BridgeEventStatus::RolledBack => {
                return Err(CreditBridgeError::AlreadyCompleted);
            }
            _ => {}
        }

        self.events[idx].status = BridgeEventStatus::Failed(reason);
        self.pending_count = self.pending_count.saturating_sub(1);

        Ok(())
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    /// Retrieve a bridge event by its ID, or `None` if not found.
    pub fn get_event(&self, id: &Hash) -> Option<&CreditBridgeEvent> {
        self.find_index(id).map(|i| &self.events[i])
    }

    /// Return all bridge events for a given user (in insertion order).
    pub fn get_user_events(&self, user: &Address) -> Vec<&CreditBridgeEvent> {
        self.by_user
            .get(user)
            .map(|indices| indices.iter().map(|&i| &self.events[i]).collect())
            .unwrap_or_default()
    }

    /// Return the number of bridge events that are still pending (non-terminal).
    pub fn get_pending_count(&self) -> usize {
        self.pending_count
    }

    /// Return aggregate bridge statistics as `(total_credits_bridged, total_isa_bridged)`.
    pub fn get_stats(&self) -> (Amount, Amount) {
        (self.total_credits_bridged, self.total_isa_bridged)
    }

    // ----------------------------------------------------------------
    // Private helpers
    // ----------------------------------------------------------------

    /// Insert a new event, updating the user index and pending counter.
    fn insert_event(&mut self, event: CreditBridgeEvent) {
        let idx = self.events.len();
        let user = event.user;
        self.events.push(event);
        self.by_user.entry(user).or_default().push(idx);
        self.pending_count += 1;
    }

    /// Linear scan for the index of an event by its ID.
    ///
    /// Events are expected to be small in number per block, so a linear scan
    /// is acceptable.  A future optimisation could add a `HashMap<Hash, usize>`
    /// index for O(1) lookup.
    fn find_index(&self, id: &Hash) -> Option<usize> {
        self.events.iter().position(|e| &e.id == id)
    }

    /// Derive a deterministic event ID from the user, ISA amount, credit amount, and height.
    fn derive_id(
        user: Address,
        isa_amount: Amount,
        credit_amount: Amount,
        height: BlockHeight,
    ) -> Hash {
        let mut data = Vec::with_capacity(20 + 16 + 16 + 8);
        data.extend_from_slice(user.as_bytes());
        data.extend_from_slice(&isa_amount.to_le_bytes());
        data.extend_from_slice(&credit_amount.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
        Hash::hash_data(&data)
    }
}

impl Default for CreditBridge {
    fn default() -> Self {
        Self::new()
    }
}

// ====================================================================
// Tests
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // Fixtures
    // ----------------------------------------------------------------

    fn user() -> Address {
        Address::from([0xAA; 20])
    }

    fn user2() -> Address {
        Address::from([0xBB; 20])
    }

    fn unknown_user() -> Address {
        Address::from([0xFF; 20])
    }

    /// 2 ISA in micro-ISA
    const TWO_ISA: Amount = 2_000_000;

    /// 100 credits
    const HUNDRED_CREDITS: Amount = 100;

    fn setup() -> CreditBridge {
        CreditBridge::new()
    }

    // ----------------------------------------------------------------
    // test_initiate_purchase
    // ----------------------------------------------------------------

    #[test]
    fn test_initiate_purchase() {
        let mut bridge = setup();

        let id = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
            .unwrap();

        let event = bridge.get_event(&id).unwrap();
        assert_eq!(event.user, user());
        assert_eq!(event.isa_amount, TWO_ISA);
        assert_eq!(event.credit_amount, HUNDRED_CREDITS);
        assert_eq!(event.event_type, BridgeEventType::CreditPurchase);
        assert_eq!(event.direction, BridgeDirection::OnChainToOffChain);
        assert_eq!(event.height, 10);
        assert_eq!(event.status, BridgeEventStatus::Initiated);
    }

    // ----------------------------------------------------------------
    // test_initiate_redemption
    // ----------------------------------------------------------------

    #[test]
    fn test_initiate_redemption() {
        let mut bridge = setup();

        let id = bridge
            .initiate_redemption(user(), HUNDRED_CREDITS, TWO_ISA, 20)
            .unwrap();

        let event = bridge.get_event(&id).unwrap();
        assert_eq!(event.user, user());
        assert_eq!(event.credit_amount, HUNDRED_CREDITS);
        assert_eq!(event.isa_amount, TWO_ISA);
        assert_eq!(event.event_type, BridgeEventType::CreditRedemption);
        assert_eq!(event.direction, BridgeDirection::OffChainToOnChain);
        assert_eq!(event.status, BridgeEventStatus::Initiated);
    }

    // ----------------------------------------------------------------
    // test_complete_event
    // ----------------------------------------------------------------

    #[test]
    fn test_complete_event() {
        let mut bridge = setup();

        let id = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
            .unwrap();

        bridge.complete_event(&id).unwrap();

        let event = bridge.get_event(&id).unwrap();
        assert_eq!(event.status, BridgeEventStatus::Completed);
    }

    // ----------------------------------------------------------------
    // test_fail_event
    // ----------------------------------------------------------------

    #[test]
    fn test_fail_event() {
        let mut bridge = setup();

        let id = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
            .unwrap();

        bridge.fail_event(&id, "oracle timeout".to_string()).unwrap();

        let event = bridge.get_event(&id).unwrap();
        assert_eq!(
            event.status,
            BridgeEventStatus::Failed("oracle timeout".to_string())
        );
    }

    // ----------------------------------------------------------------
    // test_complete_updates_stats
    // ----------------------------------------------------------------

    #[test]
    fn test_complete_updates_stats() {
        let mut bridge = setup();

        let id1 = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 10)
            .unwrap();
        let id2 = bridge
            .initiate_redemption(user2(), 200, 4_000_000, 11)
            .unwrap();

        bridge.complete_event(&id1).unwrap();
        bridge.complete_event(&id2).unwrap();

        let (credits, isa) = bridge.get_stats();
        assert_eq!(credits, HUNDRED_CREDITS + 200);
        assert_eq!(isa, TWO_ISA + 4_000_000);
    }

    // ----------------------------------------------------------------
    // test_pending_count
    // ----------------------------------------------------------------

    #[test]
    fn test_pending_count() {
        let mut bridge = setup();

        assert_eq!(bridge.get_pending_count(), 0);

        let id1 = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
            .unwrap();
        assert_eq!(bridge.get_pending_count(), 1);

        let _id2 = bridge
            .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 2)
            .unwrap();
        assert_eq!(bridge.get_pending_count(), 2);

        bridge.complete_event(&id1).unwrap();
        assert_eq!(bridge.get_pending_count(), 1);
    }

    // ----------------------------------------------------------------
    // test_pending_count_on_failure
    // ----------------------------------------------------------------

    #[test]
    fn test_pending_count_on_failure() {
        let mut bridge = setup();

        let id = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
            .unwrap();
        assert_eq!(bridge.get_pending_count(), 1);

        bridge.fail_event(&id, "network error".to_string()).unwrap();
        assert_eq!(bridge.get_pending_count(), 0);
    }

    // ----------------------------------------------------------------
    // test_already_completed_error
    // ----------------------------------------------------------------

    #[test]
    fn test_already_completed_error() {
        let mut bridge = setup();

        let id = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
            .unwrap();

        bridge.complete_event(&id).unwrap();

        // Completing again must fail
        assert_eq!(
            bridge.complete_event(&id),
            Err(CreditBridgeError::AlreadyCompleted)
        );

        // Failing a completed event must also fail
        assert_eq!(
            bridge.fail_event(&id, "late".to_string()),
            Err(CreditBridgeError::AlreadyCompleted)
        );
    }

    // ----------------------------------------------------------------
    // test_event_not_found
    // ----------------------------------------------------------------

    #[test]
    fn test_event_not_found() {
        let mut bridge = setup();
        let phantom = Hash::hash_data(b"nonexistent");

        assert_eq!(
            bridge.complete_event(&phantom),
            Err(CreditBridgeError::EventNotFound)
        );
        assert_eq!(
            bridge.fail_event(&phantom, "gone".to_string()),
            Err(CreditBridgeError::EventNotFound)
        );
        assert!(bridge.get_event(&phantom).is_none());
    }

    // ----------------------------------------------------------------
    // test_invalid_amount_purchase
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_amount_purchase() {
        let mut bridge = setup();

        assert_eq!(
            bridge.initiate_purchase(user(), 0, HUNDRED_CREDITS, 1),
            Err(CreditBridgeError::InvalidAmount)
        );

        assert_eq!(
            bridge.initiate_purchase(user(), TWO_ISA, 0, 1),
            Err(CreditBridgeError::InvalidAmount)
        );
    }

    // ----------------------------------------------------------------
    // test_invalid_amount_redemption
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_amount_redemption() {
        let mut bridge = setup();

        assert_eq!(
            bridge.initiate_redemption(user(), 0, TWO_ISA, 1),
            Err(CreditBridgeError::InvalidAmount)
        );

        assert_eq!(
            bridge.initiate_redemption(user(), HUNDRED_CREDITS, 0, 1),
            Err(CreditBridgeError::InvalidAmount)
        );
    }

    // ----------------------------------------------------------------
    // test_get_user_events
    // ----------------------------------------------------------------

    #[test]
    fn test_get_user_events() {
        let mut bridge = setup();

        bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
            .unwrap();
        bridge
            .initiate_redemption(user(), HUNDRED_CREDITS, TWO_ISA, 2)
            .unwrap();
        bridge
            .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 3)
            .unwrap();

        let user_events = bridge.get_user_events(&user());
        assert_eq!(user_events.len(), 2);
        assert_eq!(user_events[0].event_type, BridgeEventType::CreditPurchase);
        assert_eq!(user_events[1].event_type, BridgeEventType::CreditRedemption);

        let user2_events = bridge.get_user_events(&user2());
        assert_eq!(user2_events.len(), 1);

        // Unknown user returns empty
        assert!(bridge.get_user_events(&unknown_user()).is_empty());
    }

    // ----------------------------------------------------------------
    // test_stats_only_count_completed
    // ----------------------------------------------------------------

    #[test]
    fn test_stats_only_count_completed() {
        let mut bridge = setup();

        let id1 = bridge
            .initiate_purchase(user(), TWO_ISA, HUNDRED_CREDITS, 1)
            .unwrap();
        let id2 = bridge
            .initiate_purchase(user2(), TWO_ISA, HUNDRED_CREDITS, 2)
            .unwrap();

        // Complete only the first
        bridge.complete_event(&id1).unwrap();

        // Fail the second — should NOT count in stats
        bridge
            .fail_event(&id2, "rejected".to_string())
            .unwrap();

        let (credits, isa) = bridge.get_stats();
        assert_eq!(credits, HUNDRED_CREDITS); // only id1
        assert_eq!(isa, TWO_ISA);             // only id1
    }

    // ----------------------------------------------------------------
    // test_get_stats_initial
    // ----------------------------------------------------------------

    #[test]
    fn test_get_stats_initial() {
        let bridge = setup();
        assert_eq!(bridge.get_stats(), (0, 0));
        assert_eq!(bridge.get_pending_count(), 0);
    }
}
