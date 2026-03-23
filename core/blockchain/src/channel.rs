use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Errors
// ============================================================================

/// Errors related to payment channel operations
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ChannelError {
    #[error("Channel not found: {0:?}")]
    ChannelNotFound(Hash),

    #[error("Channel is not in Open status")]
    ChannelNotOpen,

    #[error("Channel has expired")]
    ChannelExpired,

    #[error("Insufficient deposit for channel operation")]
    InsufficientDeposit,

    #[error("Invalid balances: sender + receiver must equal deposit")]
    InvalidBalances,

    #[error("Invalid nonce: nonce must be strictly increasing")]
    InvalidNonce,

    #[error("Address {0:?} is not a participant in this channel")]
    NotParticipant(Address),

    #[error("Dispute period has not ended yet")]
    DisputePeriodNotOver,

    #[error("Sender and receiver cannot be the same address")]
    SameParticipants,
}

// ============================================================================
// ChannelStatus
// ============================================================================

/// The lifecycle status of a payment channel
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChannelStatus {
    /// Channel is open and accepting off-chain updates
    Open,
    /// Cooperative or unilateral close is in progress
    Closing { initiated_at: BlockHeight },
    /// A dispute has been filed and is awaiting resolution
    Disputed { disputed_at: BlockHeight },
    /// Channel has been finalised and funds disbursed
    Closed,
    /// Channel passed its expiry block without being closed
    Expired,
}

// ============================================================================
// Channel
// ============================================================================

/// A two-party state channel for high-frequency micropayments
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    /// Unique channel identifier (derived from sender, receiver, open height)
    pub id: Hash,
    /// The party that deposits funds (payer / user / agent)
    pub sender: Address,
    /// The party that accumulates funds (payee / provider / agent)
    pub receiver: Address,
    /// Total amount deposited by the sender
    pub deposit: Amount,
    /// Sender's remaining balance inside the channel
    pub sender_balance: Amount,
    /// Accumulated balance owed to the receiver
    pub receiver_balance: Amount,
    /// Monotonically-increasing state-update counter
    pub nonce: u64,
    /// Current lifecycle status
    pub status: ChannelStatus,
    /// Block height at which the channel was opened
    pub opened_at: BlockHeight,
    /// Block height at which the channel auto-expires if not closed
    pub expires_at: BlockHeight,
    /// Block height of the most recent state update
    pub last_updated: BlockHeight,
    /// Number of blocks that must pass after a close/dispute before finalisation
    pub dispute_period: u64,
}

// ============================================================================
// ChannelUpdate
// ============================================================================

/// An off-chain (or on-chain dispute) state update for a channel
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChannelUpdate {
    /// The channel this update targets
    pub channel_id: Hash,
    /// New nonce — must be strictly greater than the current channel nonce
    pub nonce: u64,
    /// Proposed sender balance after the update
    pub sender_balance: Amount,
    /// Proposed receiver balance after the update
    pub receiver_balance: Amount,
}

// ============================================================================
// ChannelManager
// ============================================================================

/// Manages all payment channels on-chain
pub struct ChannelManager {
    /// All channels keyed by channel ID
    pub channels: HashMap<Hash, Channel>,
    /// Index of channel IDs by sender address
    pub channels_by_sender: HashMap<Address, Vec<Hash>>,
    /// Index of channel IDs by receiver address
    pub channels_by_receiver: HashMap<Address, Vec<Hash>>,
    /// Default number of blocks to wait before finalisation after a close/dispute
    pub default_dispute_period: u64,
    /// Default number of blocks from open height before the channel auto-expires
    pub default_expiry: u64,
}

impl ChannelManager {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Create a new `ChannelManager`.
    ///
    /// - `default_dispute_period`: blocks to wait before finalisation (e.g. 50)
    /// - `default_expiry`: blocks from open height until auto-expiry (e.g. 10 000)
    pub fn new(default_dispute_period: u64, default_expiry: u64) -> Self {
        ChannelManager {
            channels: HashMap::new(),
            channels_by_sender: HashMap::new(),
            channels_by_receiver: HashMap::new(),
            default_dispute_period,
            default_expiry,
        }
    }

    // -----------------------------------------------------------------------
    // Core operations
    // -----------------------------------------------------------------------

    /// Open a new payment channel between `sender` and `receiver`.
    ///
    /// The channel ID is derived deterministically from the sender address,
    /// receiver address, and the block height at which the channel is opened.
    ///
    /// Returns the new channel's `Hash` ID on success.
    pub fn open_channel(
        &mut self,
        sender: Address,
        receiver: Address,
        deposit: Amount,
        height: BlockHeight,
    ) -> Result<Hash, ChannelError> {
        if sender == receiver {
            return Err(ChannelError::SameParticipants);
        }
        if deposit == 0 {
            return Err(ChannelError::InsufficientDeposit);
        }

        // Derive deterministic channel ID
        let mut preimage = Vec::with_capacity(20 + 20 + 8);
        preimage.extend_from_slice(sender.as_bytes());
        preimage.extend_from_slice(receiver.as_bytes());
        preimage.extend_from_slice(&height.to_le_bytes());
        let channel_id = Hash::hash_data(&preimage);

        let channel = Channel {
            id: channel_id,
            sender,
            receiver,
            deposit,
            sender_balance: deposit,
            receiver_balance: 0,
            nonce: 0,
            status: ChannelStatus::Open,
            opened_at: height,
            expires_at: height + self.default_expiry,
            last_updated: height,
            dispute_period: self.default_dispute_period,
        };

        self.channels.insert(channel_id, channel);
        self.channels_by_sender
            .entry(sender)
            .or_default()
            .push(channel_id);
        self.channels_by_receiver
            .entry(receiver)
            .or_default()
            .push(channel_id);

        Ok(channel_id)
    }

    /// Apply an off-chain state update to the channel on-chain.
    ///
    /// Validates that:
    /// - The channel exists and is `Open`
    /// - The new nonce is strictly greater than the current nonce
    /// - `sender_balance + receiver_balance == deposit`
    pub fn update_channel(
        &mut self,
        channel_id: &Hash,
        update: ChannelUpdate,
    ) -> Result<(), ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        if channel.status != ChannelStatus::Open {
            return Err(ChannelError::ChannelNotOpen);
        }
        if update.nonce <= channel.nonce {
            return Err(ChannelError::InvalidNonce);
        }
        if update.sender_balance + update.receiver_balance != channel.deposit {
            return Err(ChannelError::InvalidBalances);
        }

        channel.nonce = update.nonce;
        channel.sender_balance = update.sender_balance;
        channel.receiver_balance = update.receiver_balance;

        Ok(())
    }

    /// Initiate a cooperative (or unilateral) close of the channel.
    ///
    /// Only the sender or receiver may initiate. The channel must be `Open`.
    /// Transitions the channel to `Closing { initiated_at: height }`.
    pub fn close_channel(
        &mut self,
        channel_id: &Hash,
        initiator: &Address,
        height: BlockHeight,
    ) -> Result<(), ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        if *initiator != channel.sender && *initiator != channel.receiver {
            return Err(ChannelError::NotParticipant(*initiator));
        }
        if channel.status != ChannelStatus::Open {
            return Err(ChannelError::ChannelNotOpen);
        }

        channel.status = ChannelStatus::Closing { initiated_at: height };
        channel.last_updated = height;

        Ok(())
    }

    /// Submit a dispute with the latest known state.
    ///
    /// The disputer must be a channel participant. Validates the same balance
    /// invariant and nonce ordering as `update_channel`.
    /// Transitions the channel to `Disputed { disputed_at: height }` and
    /// applies the state from the update.
    pub fn dispute_channel(
        &mut self,
        channel_id: &Hash,
        update: ChannelUpdate,
        disputer: &Address,
        height: BlockHeight,
    ) -> Result<(), ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        if *disputer != channel.sender && *disputer != channel.receiver {
            return Err(ChannelError::NotParticipant(*disputer));
        }

        // A dispute can be raised from Open or Closing states
        match &channel.status {
            ChannelStatus::Open | ChannelStatus::Closing { .. } => {}
            _ => return Err(ChannelError::ChannelNotOpen),
        }

        if update.nonce <= channel.nonce {
            return Err(ChannelError::InvalidNonce);
        }
        if update.sender_balance + update.receiver_balance != channel.deposit {
            return Err(ChannelError::InvalidBalances);
        }

        channel.nonce = update.nonce;
        channel.sender_balance = update.sender_balance;
        channel.receiver_balance = update.receiver_balance;
        channel.status = ChannelStatus::Disputed { disputed_at: height };
        channel.last_updated = height;

        Ok(())
    }

    /// Finalise the channel after the dispute period has elapsed.
    ///
    /// Can be called after a `Closing` or `Disputed` transition once
    /// `current_height >= initiated_at/disputed_at + dispute_period`.
    ///
    /// Returns `(sender_refund, receiver_payout)`.
    pub fn finalize_channel(
        &mut self,
        channel_id: &Hash,
        current_height: BlockHeight,
    ) -> Result<(Amount, Amount), ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        let initiated_at = match &channel.status {
            ChannelStatus::Closing { initiated_at } => *initiated_at,
            ChannelStatus::Disputed { disputed_at } => *disputed_at,
            _ => return Err(ChannelError::ChannelNotOpen),
        };

        if current_height < initiated_at + channel.dispute_period {
            return Err(ChannelError::DisputePeriodNotOver);
        }

        let sender_refund = channel.sender_balance;
        let receiver_payout = channel.receiver_balance;

        channel.status = ChannelStatus::Closed;
        channel.last_updated = current_height;

        Ok((sender_refund, receiver_payout))
    }

    /// Add more funds to the sender's side of an open channel.
    pub fn top_up(
        &mut self,
        channel_id: &Hash,
        amount: Amount,
    ) -> Result<(), ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        if channel.status != ChannelStatus::Open {
            return Err(ChannelError::ChannelNotOpen);
        }
        if amount == 0 {
            return Err(ChannelError::InsufficientDeposit);
        }

        channel.deposit += amount;
        channel.sender_balance += amount;

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Queries
    // -----------------------------------------------------------------------

    /// Return a reference to the channel with the given ID, or `None`.
    pub fn get_channel(&self, channel_id: &Hash) -> Option<&Channel> {
        self.channels.get(channel_id)
    }

    /// Return all channels where `sender` is the payer.
    pub fn get_sender_channels(&self, sender: &Address) -> Vec<&Channel> {
        self.channels_by_sender
            .get(sender)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.channels.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Return all channels where `receiver` is the payee.
    pub fn get_receiver_channels(&self, receiver: &Address) -> Vec<&Channel> {
        self.channels_by_receiver
            .get(receiver)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.channels.get(id))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Check whether the channel has passed its expiry height.
    ///
    /// If `current_height >= expires_at` and the channel is still `Open`,
    /// the status is updated to `Expired` and `true` is returned.
    /// Returns `false` if the channel has not yet expired.
    pub fn check_expired(
        &mut self,
        channel_id: &Hash,
        current_height: BlockHeight,
    ) -> Result<bool, ChannelError> {
        let channel = self
            .channels
            .get_mut(channel_id)
            .ok_or(ChannelError::ChannelNotFound(*channel_id))?;

        if channel.status == ChannelStatus::Open && current_height >= channel.expires_at {
            channel.status = ChannelStatus::Expired;
            channel.last_updated = current_height;
            return Ok(true);
        }

        Ok(false)
    }

    /// Return the number of channels currently in the `Open` state.
    pub fn active_channel_count(&self) -> usize {
        self.channels
            .values()
            .filter(|c| c.status == ChannelStatus::Open)
            .count()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    fn sender() -> Address {
        Address::from([0xAA; 20])
    }

    fn receiver() -> Address {
        Address::from([0xBB; 20])
    }

    fn third_party() -> Address {
        Address::from([0xCC; 20])
    }

    const DEPOSIT: Amount = 1_000;
    const OPEN_HEIGHT: BlockHeight = 100;
    const DISPUTE_PERIOD: u64 = 50;
    const DEFAULT_EXPIRY: u64 = 10_000;

    fn manager() -> ChannelManager {
        ChannelManager::new(DISPUTE_PERIOD, DEFAULT_EXPIRY)
    }

    fn open_channel(mgr: &mut ChannelManager) -> Hash {
        mgr.open_channel(sender(), receiver(), DEPOSIT, OPEN_HEIGHT)
            .expect("open_channel failed")
    }

    // -----------------------------------------------------------------------
    // test_open_channel
    // -----------------------------------------------------------------------

    #[test]
    fn test_open_channel() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        let ch = mgr.get_channel(&id).unwrap();
        assert_eq!(ch.sender, sender());
        assert_eq!(ch.receiver, receiver());
        assert_eq!(ch.deposit, DEPOSIT);
        assert_eq!(ch.sender_balance, DEPOSIT);
        assert_eq!(ch.receiver_balance, 0);
        assert_eq!(ch.nonce, 0);
        assert_eq!(ch.status, ChannelStatus::Open);
        assert_eq!(ch.opened_at, OPEN_HEIGHT);
        assert_eq!(ch.expires_at, OPEN_HEIGHT + DEFAULT_EXPIRY);
        assert_eq!(ch.dispute_period, DISPUTE_PERIOD);
    }

    // -----------------------------------------------------------------------
    // test_open_same_participants_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_open_same_participants_fails() {
        let mut mgr = manager();
        let result = mgr.open_channel(sender(), sender(), DEPOSIT, OPEN_HEIGHT);
        assert_eq!(result, Err(ChannelError::SameParticipants));
    }

    // -----------------------------------------------------------------------
    // test_update_channel
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_channel() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        let update = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 700,
            receiver_balance: 300,
        };
        mgr.update_channel(&id, update).unwrap();

        let ch = mgr.get_channel(&id).unwrap();
        assert_eq!(ch.nonce, 1);
        assert_eq!(ch.sender_balance, 700);
        assert_eq!(ch.receiver_balance, 300);
    }

    // -----------------------------------------------------------------------
    // test_update_invalid_nonce_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_invalid_nonce_fails() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        // First update: nonce 1 — OK
        let update1 = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 800,
            receiver_balance: 200,
        };
        mgr.update_channel(&id, update1).unwrap();

        // Second update with same nonce — should fail
        let update2 = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 600,
            receiver_balance: 400,
        };
        let result = mgr.update_channel(&id, update2);
        assert_eq!(result, Err(ChannelError::InvalidNonce));
    }

    // -----------------------------------------------------------------------
    // test_update_invalid_balances_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_update_invalid_balances_fails() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        // sender_balance + receiver_balance != deposit (1_000)
        let update = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 500,
            receiver_balance: 400, // 500 + 400 = 900 ≠ 1_000
        };
        let result = mgr.update_channel(&id, update);
        assert_eq!(result, Err(ChannelError::InvalidBalances));
    }

    // -----------------------------------------------------------------------
    // test_close_channel
    // -----------------------------------------------------------------------

    #[test]
    fn test_close_channel() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        mgr.close_channel(&id, &sender(), 200).unwrap();

        let ch = mgr.get_channel(&id).unwrap();
        assert_eq!(ch.status, ChannelStatus::Closing { initiated_at: 200 });
    }

    // -----------------------------------------------------------------------
    // test_finalize_after_dispute_period
    // -----------------------------------------------------------------------

    #[test]
    fn test_finalize_after_dispute_period() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        // Partial payment: 300 to receiver
        let update = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 700,
            receiver_balance: 300,
        };
        mgr.update_channel(&id, update).unwrap();

        // Initiate close at block 200
        mgr.close_channel(&id, &sender(), 200).unwrap();

        // Finalise after dispute period (200 + 50 = 250)
        let (sender_refund, receiver_payout) = mgr.finalize_channel(&id, 250).unwrap();

        assert_eq!(sender_refund, 700);
        assert_eq!(receiver_payout, 300);
        assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Closed);
    }

    // -----------------------------------------------------------------------
    // test_finalize_before_period_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_finalize_before_period_fails() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        mgr.close_channel(&id, &sender(), 200).unwrap();

        // Try to finalise one block too early (200 + 50 - 1 = 249)
        let result = mgr.finalize_channel(&id, 249);
        assert_eq!(result, Err(ChannelError::DisputePeriodNotOver));
    }

    // -----------------------------------------------------------------------
    // test_dispute_channel
    // -----------------------------------------------------------------------

    #[test]
    fn test_dispute_channel() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        // Apply update nonce 1
        let update1 = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 600,
            receiver_balance: 400,
        };
        mgr.update_channel(&id, update1).unwrap();

        // Sender tries to close at nonce 1 (stale state favourable to them)
        mgr.close_channel(&id, &sender(), 200).unwrap();

        // Receiver disputes with a higher nonce
        let dispute_update = ChannelUpdate {
            channel_id: id,
            nonce: 2,
            sender_balance: 200,
            receiver_balance: 800,
        };
        mgr.dispute_channel(&id, dispute_update, &receiver(), 210)
            .unwrap();

        let ch = mgr.get_channel(&id).unwrap();
        assert_eq!(ch.status, ChannelStatus::Disputed { disputed_at: 210 });
        assert_eq!(ch.nonce, 2);
        assert_eq!(ch.sender_balance, 200);
        assert_eq!(ch.receiver_balance, 800);

        // Finalise after dispute period
        let (refund, payout) = mgr.finalize_channel(&id, 260).unwrap();
        assert_eq!(refund, 200);
        assert_eq!(payout, 800);
    }

    // -----------------------------------------------------------------------
    // test_top_up
    // -----------------------------------------------------------------------

    #[test]
    fn test_top_up() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        mgr.top_up(&id, 500).unwrap();

        let ch = mgr.get_channel(&id).unwrap();
        assert_eq!(ch.deposit, DEPOSIT + 500);
        assert_eq!(ch.sender_balance, DEPOSIT + 500);
        assert_eq!(ch.receiver_balance, 0);
    }

    // -----------------------------------------------------------------------
    // test_channel_expiry
    // -----------------------------------------------------------------------

    #[test]
    fn test_channel_expiry() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        // Not expired yet
        let still_open = mgr.check_expired(&id, OPEN_HEIGHT + DEFAULT_EXPIRY - 1).unwrap();
        assert!(!still_open);
        assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Open);

        // Exactly at expiry height — should expire
        let expired = mgr.check_expired(&id, OPEN_HEIGHT + DEFAULT_EXPIRY).unwrap();
        assert!(expired);
        assert_eq!(mgr.get_channel(&id).unwrap().status, ChannelStatus::Expired);
    }

    // -----------------------------------------------------------------------
    // test_not_participant_fails
    // -----------------------------------------------------------------------

    #[test]
    fn test_not_participant_fails() {
        let mut mgr = manager();
        let id = open_channel(&mut mgr);

        let result = mgr.close_channel(&id, &third_party(), 200);
        assert_eq!(result, Err(ChannelError::NotParticipant(third_party())));

        let dispute_update = ChannelUpdate {
            channel_id: id,
            nonce: 1,
            sender_balance: 500,
            receiver_balance: 500,
        };
        let result2 = mgr.dispute_channel(&id, dispute_update, &third_party(), 200);
        assert_eq!(result2, Err(ChannelError::NotParticipant(third_party())));
    }

    // -----------------------------------------------------------------------
    // test_get_sender_receiver_channels
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_sender_receiver_channels() {
        let mut mgr = manager();

        // Open two channels: sender → receiver (heights 100 and 200 to get distinct IDs)
        let id1 = mgr
            .open_channel(sender(), receiver(), 1_000, 100)
            .unwrap();
        let id2 = mgr
            .open_channel(sender(), receiver(), 500, 200)
            .unwrap();

        let sender_chans = mgr.get_sender_channels(&sender());
        assert_eq!(sender_chans.len(), 2);
        let ids: Vec<Hash> = sender_chans.iter().map(|c| c.id).collect();
        assert!(ids.contains(&id1));
        assert!(ids.contains(&id2));

        let receiver_chans = mgr.get_receiver_channels(&receiver());
        assert_eq!(receiver_chans.len(), 2);

        // Third party has no channels
        assert!(mgr.get_sender_channels(&third_party()).is_empty());
        assert!(mgr.get_receiver_channels(&third_party()).is_empty());

        // Active count: 2 open channels
        assert_eq!(mgr.active_channel_count(), 2);

        // Close one and verify count drops
        mgr.close_channel(&id1, &sender(), 300).unwrap();
        assert_eq!(mgr.active_channel_count(), 1);
    }
}
