//! Reputation Oracle — On-Chain Agent Performance Tracking
//!
//! This module tracks agent reputation and performance metrics on-chain,
//! enabling trust signals for agent discovery and job matching.

use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ============================================================================
// Score constants (basis points)
// ============================================================================

const SUCCESS_DELTA: i32 = 50;
const FAILURE_DELTA: i32 = -100;
const DISPUTE_WIN_DELTA: i32 = 200;
const DISPUTE_LOSS_DELTA: i32 = -300;
const MAX_SCORE: u32 = 10_000;

// ============================================================================
// Errors
// ============================================================================

/// Errors for the reputation system
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ReputationError {
    #[error("Agent not found: {0:?}")]
    AgentNotFound(Hash),

    #[error("Invalid score: score must be in 0–10 000 basis points")]
    InvalidScore,

    #[error("Unauthorized updater: {0}")]
    UnauthorizedUpdater(Address),

    #[error("Agent already registered: {0:?}")]
    AlreadyRegistered(Hash),
}

// ============================================================================
// Supporting Types
// ============================================================================

/// Kind of event that changed an agent's reputation
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReputationEventType {
    JobCompleted,
    JobFailed,
    DisputeWon,
    DisputeLost,
    SlashPenalty,
    BonusReward,
    PeerReview,
}

/// A single scored event in an agent's reputation history
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationEvent {
    /// What caused the change
    pub event_type: ReputationEventType,
    /// Score delta in basis points (may be negative)
    pub delta: i32,
    /// Block at which the event occurred
    pub height: BlockHeight,
    /// Free-form description or reference
    pub details: String,
}

/// Full on-chain reputation record for an agent
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReputationRecord {
    /// Unique agent identifier
    pub agent_id: Hash,
    /// Current score in basis points (0–10 000)
    pub score: u32,
    /// Total number of interactions (success + failure)
    pub total_interactions: u64,
    /// Number of successfully completed interactions
    pub successful_interactions: u64,
    /// Number of failed interactions
    pub failed_interactions: u64,
    /// Cumulative revenue from successful interactions
    pub total_revenue: Amount,
    /// Exponential moving average of response time
    pub average_response_time_ms: u64,
    /// Block at which the record was last touched
    pub last_updated: BlockHeight,
    /// Full audit trail of score-changing events
    pub history: Vec<ReputationEvent>,
}

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

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Helpers -----------------------------------------------------------------

    fn make_address(seed: u8) -> Address {
        Address::new([seed; 20])
    }

    fn make_hash(seed: u8) -> Hash {
        Hash::new([seed; 32])
    }

    fn default_system() -> ReputationSystem {
        ReputationSystem::new(make_address(0x01), 5000)
    }

    // Tests -------------------------------------------------------------------

    #[test]
    fn test_register_agent() {
        let mut sys = default_system();
        let id = make_hash(0x10);

        sys.register_agent(id).unwrap();

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.agent_id, id);
        assert_eq!(rec.score, 5000);
        assert_eq!(rec.total_interactions, 0);
    }

    #[test]
    fn test_duplicate_register() {
        let mut sys = default_system();
        let id = make_hash(0x10);

        sys.register_agent(id).unwrap();
        let err = sys.register_agent(id).unwrap_err();
        assert_eq!(err, ReputationError::AlreadyRegistered(id));
    }

    #[test]
    fn test_record_success() {
        let mut sys = default_system();
        let id = make_hash(0x11);
        sys.register_agent(id).unwrap();

        let new_score = sys.record_success(&id, 1000, 200, 10).unwrap();
        assert_eq!(new_score, 5050); // 5000 + 50

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.successful_interactions, 1);
        assert_eq!(rec.total_revenue, 1000);
        assert_eq!(rec.history.len(), 1);
        assert_eq!(rec.history[0].event_type, ReputationEventType::JobCompleted);
    }

    #[test]
    fn test_record_failure() {
        let mut sys = default_system();
        let id = make_hash(0x12);
        sys.register_agent(id).unwrap();

        let new_score = sys
            .record_failure(&id, 10, "timeout".to_string())
            .unwrap();
        assert_eq!(new_score, 4900); // 5000 - 100

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.failed_interactions, 1);
        assert_eq!(rec.history[0].event_type, ReputationEventType::JobFailed);
    }

    #[test]
    fn test_score_capping_at_max() {
        let mut sys = ReputationSystem::new(make_address(0x01), 9990);
        let id = make_hash(0x13);
        sys.register_agent(id).unwrap();

        // +50 would exceed 10 000, should cap
        let new_score = sys.record_success(&id, 0, 0, 1).unwrap();
        assert_eq!(new_score, MAX_SCORE);
        assert!(new_score <= MAX_SCORE);
    }

    #[test]
    fn test_score_floor_at_zero() {
        let mut sys = ReputationSystem::new(make_address(0x01), 50);
        let id = make_hash(0x14);
        sys.register_agent(id).unwrap();

        // -100 from 50 would go negative, should floor at 0
        let new_score = sys
            .record_failure(&id, 1, "network error".to_string())
            .unwrap();
        assert_eq!(new_score, 0);
    }

    #[test]
    fn test_dispute_won() {
        let mut sys = default_system();
        let id = make_hash(0x15);
        sys.register_agent(id).unwrap();

        let new_score = sys.record_dispute(&id, true, 5).unwrap();
        assert_eq!(new_score, 5200); // 5000 + 200

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.history[0].event_type, ReputationEventType::DisputeWon);
        assert_eq!(rec.history[0].delta, DISPUTE_WIN_DELTA);
    }

    #[test]
    fn test_dispute_lost() {
        let mut sys = default_system();
        let id = make_hash(0x16);
        sys.register_agent(id).unwrap();

        let new_score = sys.record_dispute(&id, false, 5).unwrap();
        assert_eq!(new_score, 4700); // 5000 - 300

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.history[0].event_type, ReputationEventType::DisputeLost);
        assert_eq!(rec.history[0].delta, DISPUTE_LOSS_DELTA);
    }

    #[test]
    fn test_apply_slash() {
        let mut sys = default_system();
        let id = make_hash(0x17);
        sys.register_agent(id).unwrap();

        let new_score = sys.apply_slash(&id, 500, 10).unwrap();
        assert_eq!(new_score, 4500); // 5000 - 500

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.history[0].event_type, ReputationEventType::SlashPenalty);
    }

    #[test]
    fn test_apply_bonus() {
        let mut sys = default_system();
        let id = make_hash(0x18);
        sys.register_agent(id).unwrap();

        let new_score = sys
            .apply_bonus(&id, 300, 10, "top performer".to_string())
            .unwrap();
        assert_eq!(new_score, 5300); // 5000 + 300

        let rec = sys.get_reputation(&id).unwrap();
        assert_eq!(rec.history[0].event_type, ReputationEventType::BonusReward);
        assert_eq!(rec.history[0].details, "top performer");
    }

    #[test]
    fn test_get_top_agents() {
        let mut sys = default_system();

        // Register 5 agents with different initial scores
        for i in 0u8..5 {
            let id = make_hash(i);
            let mut agent_sys = ReputationSystem::new(make_address(0x01), (i as u32) * 1000);
            agent_sys.register_agent(id).unwrap();
            // Copy the record into our main system
            let rec = agent_sys.records.remove(&id).unwrap();
            sys.records.insert(id, rec);
        }

        let top3 = sys.get_top_agents(3);
        assert_eq!(top3.len(), 3);
        // Sorted by score descending
        assert!(top3[0].1 >= top3[1].1);
        assert!(top3[1].1 >= top3[2].1);
    }

    #[test]
    fn test_decay_inactive() {
        let mut sys = default_system();
        let id = make_hash(0x20);
        sys.register_agent(id).unwrap();

        // last_updated is 0; current_height is 200; threshold is 100 → agent is inactive
        sys.decay_inactive(200, 100);
        let score_after = sys.get_score(&id).unwrap();
        assert_eq!(score_after, 4990); // 5000 - 10 (decay_rate_bps)
    }

    #[test]
    fn test_decay_not_applied_to_active_agents() {
        let mut sys = default_system();
        let id = make_hash(0x21);
        sys.register_agent(id).unwrap();

        // Record a success at block 150 so last_updated = 150
        sys.record_success(&id, 100, 50, 150).unwrap();

        // current_height = 200, threshold = 100 → 200 - 150 = 50 ≤ 100 → not inactive
        sys.decay_inactive(200, 100);
        let score_after = sys.get_score(&id).unwrap();
        assert_eq!(score_after, 5050); // unchanged from the success bump
    }

    #[test]
    fn test_success_rate() {
        let mut sys = default_system();
        let id = make_hash(0x22);
        sys.register_agent(id).unwrap();

        // No interactions yet → None
        assert!(sys.success_rate(&id).is_none());

        sys.record_success(&id, 100, 50, 1).unwrap();
        sys.record_success(&id, 100, 50, 2).unwrap();
        sys.record_failure(&id, 3, "err".to_string()).unwrap();

        // 2 successes out of 3 → 6666 bps (rounded down)
        let rate = sys.success_rate(&id).unwrap();
        assert_eq!(rate, 6666);
    }

    #[test]
    fn test_agent_not_found_errors() {
        let mut sys = default_system();
        let missing = make_hash(0xFF);

        assert_eq!(
            sys.record_success(&missing, 0, 0, 1).unwrap_err(),
            ReputationError::AgentNotFound(missing)
        );
        assert_eq!(
            sys.record_failure(&missing, 1, "x".to_string()).unwrap_err(),
            ReputationError::AgentNotFound(missing)
        );
        assert_eq!(
            sys.record_dispute(&missing, true, 1).unwrap_err(),
            ReputationError::AgentNotFound(missing)
        );
        assert_eq!(
            sys.apply_slash(&missing, 100, 1).unwrap_err(),
            ReputationError::AgentNotFound(missing)
        );
        assert_eq!(
            sys.apply_bonus(&missing, 100, 1, "x".to_string()).unwrap_err(),
            ReputationError::AgentNotFound(missing)
        );
    }

    #[test]
    fn test_authorize_updater_admin_only() {
        let admin = make_address(0x01);
        let mut sys = ReputationSystem::new(admin, 5000);
        let updater = make_address(0x02);
        let impostor = make_address(0xFF);

        // Admin can authorize
        sys.authorize_updater(updater, &admin).unwrap();
        assert!(sys.authorized_updaters.contains(&updater));

        // Non-admin cannot authorize
        let err = sys.authorize_updater(updater, &impostor).unwrap_err();
        assert_eq!(err, ReputationError::UnauthorizedUpdater(impostor));
    }
}
