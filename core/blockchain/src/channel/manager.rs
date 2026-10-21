use super::types::{Channel, ChannelError, ChannelStatus, ChannelUpdate};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

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
