use crate::types::BlockHeight;
use std::collections::HashMap;

use super::types::SyncStatus;

// ============================================================================
// Sync Manager
// ============================================================================

/// Determines which blocks to request and tracks in-flight requests.
pub struct SyncManager {
    /// Our current confirmed chain height.
    pub(crate) local_height: BlockHeight,
    /// Highest height advertised by any peer.
    pub(crate) best_peer_height: BlockHeight,
    /// Current sync status.
    pub(crate) status: SyncStatus,
    /// Blocks that have been requested but not yet received:
    /// maps start_height → time of request.
    pub(crate) pending_requests: HashMap<BlockHeight, std::time::Instant>,
    /// Maximum number of outstanding block requests.
    pub(crate) max_concurrent_requests: usize,
}

impl SyncManager {
    /// Create a new `SyncManager` starting from `local_height`.
    pub fn new(local_height: BlockHeight) -> Self {
        Self {
            local_height,
            best_peer_height: local_height,
            // When no peers are known yet, local == best → Synced.
            status: SyncStatus::Synced,
            pending_requests: HashMap::new(),
            max_concurrent_requests: 16,
        }
    }

    /// Notify the manager that a peer is at `height`.
    pub fn update_peer_height(&mut self, height: BlockHeight) {
        if height > self.best_peer_height {
            self.best_peer_height = height;
        }
        self.refresh_status();
    }

    /// Current sync status.
    pub fn get_sync_status(&self) -> &SyncStatus {
        &self.status
    }

    /// `true` when our chain is behind the best known peer.
    pub fn needs_sync(&self) -> bool {
        self.local_height < self.best_peer_height
    }

    /// Returns the next `(start_height, count)` range to request, or `None` if
    /// sync is not needed or the concurrent-request limit has been reached.
    pub fn get_blocks_to_request(&mut self, batch_size: u32) -> Option<(BlockHeight, u32)> {
        if !self.needs_sync() {
            return None;
        }
        if self.pending_requests.len() >= self.max_concurrent_requests {
            return None;
        }

        // Find the lowest height above local_height that isn't already pending.
        let next = self.local_height + 1;

        // Walk forward until we find a slot not in pending_requests.
        let mut candidate = next;
        while self.pending_requests.contains_key(&candidate) {
            candidate += batch_size as BlockHeight;
        }

        if candidate > self.best_peer_height {
            return None;
        }

        // Clamp the count so we don't request past best_peer_height.
        let remaining = self.best_peer_height - candidate + 1;
        let count = (batch_size as u64).min(remaining) as u32;

        self.pending_requests
            .insert(candidate, std::time::Instant::now());

        Some((candidate, count))
    }

    /// Record that the block at `height` has been received and applied.
    pub fn mark_received(&mut self, height: BlockHeight) {
        self.pending_requests.remove(&height);
        if height > self.local_height {
            self.local_height = height;
        }
        self.refresh_status();
    }

    /// Explicitly update local chain height (e.g. after block import).
    pub fn update_local_height(&mut self, height: BlockHeight) {
        self.local_height = height;
        self.refresh_status();
    }

    /// `true` when the local chain is at or ahead of the best peer.
    pub fn is_synced(&self) -> bool {
        self.local_height >= self.best_peer_height
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    fn refresh_status(&mut self) {
        self.status = if self.local_height >= self.best_peer_height {
            SyncStatus::Synced
        } else {
            SyncStatus::Syncing {
                target_height: self.best_peer_height,
                current_height: self.local_height,
            }
        };
    }
}
