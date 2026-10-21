use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::{HashMap, HashSet};

use super::types::{
    ReputationError, ReputationEvent, ReputationEventType, ReputationRecord, DISPUTE_LOSS_DELTA,
    DISPUTE_WIN_DELTA, FAILURE_DELTA, MAX_SCORE, SUCCESS_DELTA,
};

// ============================================================================
// System
// ============================================================================

/// On-chain reputation oracle for AI agents
#[derive(Clone, Debug)]
pub struct ReputationSystem {
    /// agent_id → record
    pub records: HashMap<Hash, ReputationRecord>,
    /// Addresses permitted to record events (in addition to admin)
    pub authorized_updaters: HashSet<Address>,
    /// Admin address — may authorize updaters and register agents
    pub admin: Address,
    /// Score assigned to newly registered agents
    pub initial_score: u32,
    /// Score decay per inactivity epoch, in basis points (default 10 = 0.1 %)
    pub decay_rate_bps: u32,
}

impl ReputationSystem {
    // -------------------------------------------------------------------------
    // Construction
    // -------------------------------------------------------------------------

    /// Create a new system with the given admin and default initial score.
    pub fn new(admin: Address, initial_score: u32) -> Self {
        Self {
            records: HashMap::new(),
            authorized_updaters: HashSet::new(),
            admin,
            initial_score,
            decay_rate_bps: 10,
        }
    }

    // -------------------------------------------------------------------------
    // Registration
    // -------------------------------------------------------------------------

    /// Register a new agent in the reputation system.
    /// Returns `AlreadyRegistered` if the agent_id is already known.
    pub fn register_agent(&mut self, agent_id: Hash) -> Result<(), ReputationError> {
        if self.records.contains_key(&agent_id) {
            return Err(ReputationError::AlreadyRegistered(agent_id));
        }

        let record = ReputationRecord {
            agent_id,
            score: self.initial_score,
            total_interactions: 0,
            successful_interactions: 0,
            failed_interactions: 0,
            total_revenue: 0,
            average_response_time_ms: 0,
            last_updated: 0,
            history: Vec::new(),
        };

        self.records.insert(agent_id, record);
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Event recording
    // -------------------------------------------------------------------------

    /// Record a successful job and return the new score.
    pub fn record_success(
        &mut self,
        agent_id: &Hash,
        revenue: Amount,
        response_time_ms: u64,
        height: BlockHeight,
    ) -> Result<u32, ReputationError> {
        let record = self
            .records
            .get_mut(agent_id)
            .ok_or(ReputationError::AgentNotFound(*agent_id))?;

        record.total_interactions += 1;
        record.successful_interactions += 1;
        record.total_revenue += revenue;

        // Update EMA of response time
        record.average_response_time_ms = if record.total_interactions == 1 {
            response_time_ms
        } else {
            (record.average_response_time_ms + response_time_ms) / 2
        };

        let new_score = apply_delta(record.score, SUCCESS_DELTA);
        record.score = new_score;
        record.last_updated = height;

        record.history.push(ReputationEvent {
            event_type: ReputationEventType::JobCompleted,
            delta: SUCCESS_DELTA,
            height,
            details: format!("revenue={revenue}, response_time_ms={response_time_ms}"),
        });

        Ok(new_score)
    }

    /// Record a failed job and return the new score.
    pub fn record_failure(
        &mut self,
        agent_id: &Hash,
        height: BlockHeight,
        details: String,
    ) -> Result<u32, ReputationError> {
        let record = self
            .records
            .get_mut(agent_id)
            .ok_or(ReputationError::AgentNotFound(*agent_id))?;

        record.total_interactions += 1;
        record.failed_interactions += 1;

        let new_score = apply_delta(record.score, FAILURE_DELTA);
        record.score = new_score;
        record.last_updated = height;

        record.history.push(ReputationEvent {
            event_type: ReputationEventType::JobFailed,
            delta: FAILURE_DELTA,
            height,
            details,
        });

        Ok(new_score)
    }

    /// Record a dispute outcome and return the new score.
    pub fn record_dispute(
        &mut self,
        agent_id: &Hash,
        won: bool,
        height: BlockHeight,
    ) -> Result<u32, ReputationError> {
        let record = self
            .records
            .get_mut(agent_id)
            .ok_or(ReputationError::AgentNotFound(*agent_id))?;

        let (delta, event_type) = if won {
            (DISPUTE_WIN_DELTA, ReputationEventType::DisputeWon)
        } else {
            (DISPUTE_LOSS_DELTA, ReputationEventType::DisputeLost)
        };

        let new_score = apply_delta(record.score, delta);
        record.score = new_score;
        record.last_updated = height;

        record.history.push(ReputationEvent {
            event_type,
            delta,
            height,
            details: format!("dispute_outcome={}", if won { "won" } else { "lost" }),
        });

        Ok(new_score)
    }

    /// Apply a slash penalty (in basis points) and return the new score.
    pub fn apply_slash(
        &mut self,
        agent_id: &Hash,
        penalty_bps: u32,
        height: BlockHeight,
    ) -> Result<u32, ReputationError> {
        let record = self
            .records
            .get_mut(agent_id)
            .ok_or(ReputationError::AgentNotFound(*agent_id))?;

        let delta = -(penalty_bps as i32);
        let new_score = apply_delta(record.score, delta);
        record.score = new_score;
        record.last_updated = height;

        record.history.push(ReputationEvent {
            event_type: ReputationEventType::SlashPenalty,
            delta,
            height,
            details: format!("penalty_bps={penalty_bps}"),
        });

        Ok(new_score)
    }

    /// Apply a bonus reward (in basis points) and return the new score.
    pub fn apply_bonus(
        &mut self,
        agent_id: &Hash,
        bonus_bps: u32,
        height: BlockHeight,
        details: String,
    ) -> Result<u32, ReputationError> {
        let record = self
            .records
            .get_mut(agent_id)
            .ok_or(ReputationError::AgentNotFound(*agent_id))?;

        let delta = bonus_bps as i32;
        let new_score = apply_delta(record.score, delta);
        record.score = new_score;
        record.last_updated = height;

        record.history.push(ReputationEvent {
            event_type: ReputationEventType::BonusReward,
            delta,
            height,
            details,
        });

        Ok(new_score)
    }

    // -------------------------------------------------------------------------
    // Queries
    // -------------------------------------------------------------------------

    /// Return the full reputation record for an agent, if registered.
    pub fn get_reputation(&self, agent_id: &Hash) -> Option<&ReputationRecord> {
        self.records.get(agent_id)
    }

    /// Return the current score for an agent in basis points.
    pub fn get_score(&self, agent_id: &Hash) -> Option<u32> {
        self.records.get(agent_id).map(|r| r.score)
    }

    /// Return up to `limit` agents sorted by score descending.
    pub fn get_top_agents(&self, limit: usize) -> Vec<(&Hash, u32)> {
        let mut all: Vec<(&Hash, u32)> = self
            .records
            .iter()
            .map(|(id, rec)| (id, rec.score))
            .collect();

        all.sort_by(|a, b| b.1.cmp(&a.1));
        all.truncate(limit);
        all
    }

    /// Return the success rate in basis points (0–10 000).
    /// Returns `None` if the agent has no recorded interactions.
    pub fn success_rate(&self, agent_id: &Hash) -> Option<u32> {
        let record = self.records.get(agent_id)?;
        if record.total_interactions == 0 {
            return None;
        }
        let rate = (record.successful_interactions as u128 * MAX_SCORE as u128
            / record.total_interactions as u128) as u32;
        Some(rate)
    }

    // -------------------------------------------------------------------------
    // Administration
    // -------------------------------------------------------------------------

    /// Authorize an address to record reputation events. Admin-only.
    pub fn authorize_updater(
        &mut self,
        address: Address,
        admin: &Address,
    ) -> Result<(), ReputationError> {
        if admin != &self.admin {
            return Err(ReputationError::UnauthorizedUpdater(*admin));
        }
        self.authorized_updaters.insert(address);
        Ok(())
    }

    /// Reduce scores for agents that have been inactive for more than
    /// `inactivity_threshold` blocks relative to `current_height`.
    pub fn decay_inactive(&mut self, current_height: BlockHeight, inactivity_threshold: u64) {
        let decay = self.decay_rate_bps;
        for record in self.records.values_mut() {
            if current_height.saturating_sub(record.last_updated) > inactivity_threshold {
                record.score = record.score.saturating_sub(decay);
            }
        }
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Apply a signed delta to a score, clamping to [0, MAX_SCORE].
#[inline]
fn apply_delta(score: u32, delta: i32) -> u32 {
    let new_score = score as i64 + delta as i64;
    new_score.clamp(0, MAX_SCORE as i64) as u32
}
