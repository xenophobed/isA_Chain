use crate::types::{Address, Amount, BlockHeight, Hash};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Enums
// ============================================================================

/// Reason a reward was granted to a contributor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RewardReason {
    BugBounty,
    FeatureDevelopment,
    Documentation,
    SecurityAudit,
    CommunityContribution,
    ToolDevelopment,
    Custom(String),
}

/// Lifecycle state of an incentive program.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgramStatus {
    Active,
    Paused,
    Expired,
    Exhausted,
}

// ============================================================================
// Structs
// ============================================================================

/// A single reward disbursement within a program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reward {
    pub recipient: Address,
    pub amount: Amount,
    pub reason: RewardReason,
    pub height: BlockHeight,
}

/// A treasury-funded developer incentive program.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IncentiveProgram {
    pub id: Hash,
    pub name: String,
    /// Total budget allocated from treasury.
    pub budget: Amount,
    /// Amount already disbursed.
    pub spent: Amount,
    pub rewards: Vec<Reward>,
    pub status: ProgramStatus,
    pub created_at: BlockHeight,
    pub expires_at: BlockHeight,
    /// Address authorised to award rewards for this program.
    pub admin: Address,
}

impl IncentiveProgram {
    fn remaining(&self) -> Amount {
        self.budget.saturating_sub(self.spent)
    }
}

// ============================================================================
// Errors
// ============================================================================

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum IncentiveError {
    #[error("Program not found: {0:?}")]
    ProgramNotFound(Hash),

    #[error("Program is not active")]
    ProgramNotActive,

    #[error("Budget exhausted: remaining {remaining}, requested {requested}")]
    BudgetExhausted { remaining: Amount, requested: Amount },

    #[error("Program has expired")]
    ProgramExpired,

    #[error("Unauthorized admin: {0:?}")]
    UnauthorizedAdmin(Address),

    #[error("Amount must be greater than zero")]
    InvalidAmount,

    #[error("A program with this ID already exists")]
    DuplicateProgram,
}

// ============================================================================
// Manager
// ============================================================================

/// Manages all developer incentive programs backed by treasury funds.
pub struct IncentiveManager {
    pub programs: HashMap<Hash, IncentiveProgram>,
    pub total_rewarded: Amount,
    pub total_programs: u64,
    pub global_admin: Address,
}

impl IncentiveManager {
    // ----------------------------------------------------------------
    // Construction
    // ----------------------------------------------------------------

    pub fn new(admin: Address) -> Self {
        IncentiveManager {
            programs: HashMap::new(),
            total_rewarded: 0,
            total_programs: 0,
            global_admin: admin,
        }
    }

    // ----------------------------------------------------------------
    // Program lifecycle
    // ----------------------------------------------------------------

    /// Create a new incentive program funded from the treasury.
    ///
    /// The `id` is derived deterministically from the program name and
    /// creation height so it is unique and reproducible.
    pub fn create_program(
        &mut self,
        name: String,
        budget: Amount,
        expires_at: BlockHeight,
        admin: Address,
        height: BlockHeight,
    ) -> Result<Hash, IncentiveError> {
        if budget == 0 {
            return Err(IncentiveError::InvalidAmount);
        }

        // Deterministic ID: blake3(name || height || admin)
        let mut id_input = Vec::new();
        id_input.extend_from_slice(name.as_bytes());
        id_input.extend_from_slice(&height.to_le_bytes());
        id_input.extend_from_slice(admin.as_bytes());
        let id = Hash::hash_data(&id_input);

        if self.programs.contains_key(&id) {
            return Err(IncentiveError::DuplicateProgram);
        }

        let program = IncentiveProgram {
            id,
            name,
            budget,
            spent: 0,
            rewards: Vec::new(),
            status: ProgramStatus::Active,
            created_at: height,
            expires_at,
            admin,
        };

        self.programs.insert(id, program);
        self.total_programs += 1;
        Ok(id)
    }

    /// Award a reward to a contributor within the named program.
    ///
    /// Only the program admin (or the global admin) may call this.
    pub fn award_reward(
        &mut self,
        program_id: &Hash,
        recipient: Address,
        amount: Amount,
        reason: RewardReason,
        height: BlockHeight,
        admin: &Address,
    ) -> Result<(), IncentiveError> {
        if amount == 0 {
            return Err(IncentiveError::InvalidAmount);
        }

        let program = self
            .programs
            .get_mut(program_id)
            .ok_or(IncentiveError::ProgramNotFound(*program_id))?;

        // Auth check: program admin OR global admin.
        if *admin != program.admin && *admin != self.global_admin {
            return Err(IncentiveError::UnauthorizedAdmin(*admin));
        }

        // Expiry check before status check so it can't be awarded post-expiry.
        if height > program.expires_at && program.status != ProgramStatus::Expired {
            program.status = ProgramStatus::Expired;
        }

        match program.status {
            ProgramStatus::Active => {}
            ProgramStatus::Paused => return Err(IncentiveError::ProgramNotActive),
            ProgramStatus::Expired => return Err(IncentiveError::ProgramExpired),
            ProgramStatus::Exhausted => return Err(IncentiveError::ProgramNotActive),
        }

        let remaining = program.remaining();
        if amount > remaining {
            return Err(IncentiveError::BudgetExhausted {
                remaining,
                requested: amount,
            });
        }

        program.spent += amount;
        if program.remaining() == 0 {
            program.status = ProgramStatus::Exhausted;
        }

        program.rewards.push(Reward {
            recipient,
            amount,
            reason,
            height,
        });

        self.total_rewarded += amount;
        Ok(())
    }

    /// Pause an active program (only program admin or global admin).
    pub fn pause_program(&mut self, id: &Hash, admin: &Address) -> Result<(), IncentiveError> {
        let program = self
            .programs
            .get_mut(id)
            .ok_or(IncentiveError::ProgramNotFound(*id))?;

        if *admin != program.admin && *admin != self.global_admin {
            return Err(IncentiveError::UnauthorizedAdmin(*admin));
        }

        if program.status != ProgramStatus::Active {
            return Err(IncentiveError::ProgramNotActive);
        }

        program.status = ProgramStatus::Paused;
        Ok(())
    }

    /// Resume a paused program (only program admin or global admin).
    pub fn resume_program(&mut self, id: &Hash, admin: &Address) -> Result<(), IncentiveError> {
        let program = self
            .programs
            .get_mut(id)
            .ok_or(IncentiveError::ProgramNotFound(*id))?;

        if *admin != program.admin && *admin != self.global_admin {
            return Err(IncentiveError::UnauthorizedAdmin(*admin));
        }

        if program.status != ProgramStatus::Paused {
            return Err(IncentiveError::ProgramNotActive);
        }

        program.status = ProgramStatus::Active;
        Ok(())
    }

    /// Mark all programs whose `expires_at` is before or equal to
    /// `current_height` as `Expired` (unless already exhausted).
    pub fn expire_programs(&mut self, current_height: BlockHeight) {
        for program in self.programs.values_mut() {
            if program.expires_at < current_height
                && program.status != ProgramStatus::Exhausted
                && program.status != ProgramStatus::Expired
            {
                program.status = ProgramStatus::Expired;
            }
        }
    }

    // ----------------------------------------------------------------
    // Queries
    // ----------------------------------------------------------------

    pub fn get_program(&self, id: &Hash) -> Option<&IncentiveProgram> {
        self.programs.get(id)
    }

    /// Returns all rewards for the given program, or an empty vec if not found.
    pub fn get_rewards(&self, program_id: &Hash) -> Vec<&Reward> {
        match self.programs.get(program_id) {
            Some(p) => p.rewards.iter().collect(),
            None => vec![],
        }
    }

    /// Returns all rewards across all programs for a given recipient.
    pub fn get_recipient_rewards(&self, recipient: &Address) -> Vec<(&Hash, &Reward)> {
        let mut results = Vec::new();
        for (id, program) in &self.programs {
            for reward in &program.rewards {
                if &reward.recipient == recipient {
                    results.push((id, reward));
                }
            }
        }
        results
    }

    pub fn get_total_rewarded(&self) -> Amount {
        self.total_rewarded
    }

    /// Returns all currently active programs.
    pub fn active_programs(&self) -> Vec<&IncentiveProgram> {
        self.programs
            .values()
            .filter(|p| p.status == ProgramStatus::Active)
            .collect()
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn global_admin() -> Address {
        Address::from([0xAA; 20])
    }

    fn program_admin() -> Address {
        Address::from([0xBB; 20])
    }

    fn contributor(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    fn rando() -> Address {
        Address::from([0xFF; 20])
    }

    fn default_manager() -> IncentiveManager {
        IncentiveManager::new(global_admin())
    }

    fn create_default_program(mgr: &mut IncentiveManager) -> Hash {
        mgr.create_program(
            "Bug Bounty Q1".to_string(),
            10_000,
            1_000,
            program_admin(),
            1,
        )
        .unwrap()
    }

    // ----------------------------------------------------------------
    // test_create_program
    // ----------------------------------------------------------------

    #[test]
    fn test_create_program() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        let prog = mgr.get_program(&id).unwrap();
        assert_eq!(prog.name, "Bug Bounty Q1");
        assert_eq!(prog.budget, 10_000);
        assert_eq!(prog.spent, 0);
        assert_eq!(prog.status, ProgramStatus::Active);
        assert_eq!(prog.admin, program_admin());
        assert_eq!(mgr.total_programs, 1);
    }

    // ----------------------------------------------------------------
    // test_award_reward
    // ----------------------------------------------------------------

    #[test]
    fn test_award_reward() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        mgr.award_reward(
            &id,
            contributor(0x01),
            500,
            RewardReason::BugBounty,
            2,
            &program_admin(),
        )
        .unwrap();

        let prog = mgr.get_program(&id).unwrap();
        assert_eq!(prog.spent, 500);
        assert_eq!(prog.rewards.len(), 1);
        assert_eq!(prog.rewards[0].amount, 500);
        assert_eq!(prog.rewards[0].recipient, contributor(0x01));
        assert_eq!(mgr.get_total_rewarded(), 500);
    }

    // ----------------------------------------------------------------
    // test_budget_exhausted
    // ----------------------------------------------------------------

    #[test]
    fn test_budget_exhausted() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr); // budget = 10_000

        // Use almost all budget
        mgr.award_reward(
            &id,
            contributor(0x01),
            9_000,
            RewardReason::FeatureDevelopment,
            2,
            &program_admin(),
        )
        .unwrap();

        // Try to award more than remaining (1_000)
        let err = mgr
            .award_reward(
                &id,
                contributor(0x02),
                2_000,
                RewardReason::BugBounty,
                3,
                &program_admin(),
            )
            .unwrap_err();

        assert_eq!(
            err,
            IncentiveError::BudgetExhausted {
                remaining: 1_000,
                requested: 2_000,
            }
        );
    }

    // ----------------------------------------------------------------
    // test_program_expired
    // ----------------------------------------------------------------

    #[test]
    fn test_program_expired() {
        let mut mgr = default_manager();
        // expires_at = 5
        let id = mgr
            .create_program(
                "Short Program".to_string(),
                10_000,
                5,
                program_admin(),
                1,
            )
            .unwrap();

        // Award at height 10, which is past expiry — should auto-expire
        let err = mgr
            .award_reward(
                &id,
                contributor(0x01),
                100,
                RewardReason::Documentation,
                10,
                &program_admin(),
            )
            .unwrap_err();

        assert_eq!(err, IncentiveError::ProgramExpired);
        assert_eq!(
            mgr.get_program(&id).unwrap().status,
            ProgramStatus::Expired
        );
    }

    // ----------------------------------------------------------------
    // test_unauthorized_admin
    // ----------------------------------------------------------------

    #[test]
    fn test_unauthorized_admin() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        let err = mgr
            .award_reward(
                &id,
                contributor(0x01),
                100,
                RewardReason::BugBounty,
                2,
                &rando(),
            )
            .unwrap_err();

        assert_eq!(err, IncentiveError::UnauthorizedAdmin(rando()));
    }

    /// Global admin should be able to award rewards even if not the program admin.
    #[test]
    fn test_global_admin_can_award() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        mgr.award_reward(
            &id,
            contributor(0x01),
            100,
            RewardReason::BugBounty,
            2,
            &global_admin(),
        )
        .unwrap();

        assert_eq!(mgr.get_program(&id).unwrap().spent, 100);
    }

    // ----------------------------------------------------------------
    // test_pause_resume
    // ----------------------------------------------------------------

    #[test]
    fn test_pause_resume() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        // Pause
        mgr.pause_program(&id, &program_admin()).unwrap();
        assert_eq!(mgr.get_program(&id).unwrap().status, ProgramStatus::Paused);

        // Award while paused should fail
        let err = mgr
            .award_reward(
                &id,
                contributor(0x01),
                100,
                RewardReason::BugBounty,
                2,
                &program_admin(),
            )
            .unwrap_err();
        assert_eq!(err, IncentiveError::ProgramNotActive);

        // Resume
        mgr.resume_program(&id, &program_admin()).unwrap();
        assert_eq!(mgr.get_program(&id).unwrap().status, ProgramStatus::Active);

        // Award should succeed again
        mgr.award_reward(
            &id,
            contributor(0x01),
            100,
            RewardReason::BugBounty,
            2,
            &program_admin(),
        )
        .unwrap();
        assert_eq!(mgr.get_program(&id).unwrap().spent, 100);
    }

    // ----------------------------------------------------------------
    // test_expire_programs
    // ----------------------------------------------------------------

    #[test]
    fn test_expire_programs() {
        let mut mgr = default_manager();

        // expires_at = 10
        let id = mgr
            .create_program(
                "Expiring".to_string(),
                5_000,
                10,
                program_admin(),
                1,
            )
            .unwrap();

        // Not yet expired
        mgr.expire_programs(10);
        assert_eq!(mgr.get_program(&id).unwrap().status, ProgramStatus::Active);

        // Now expired
        mgr.expire_programs(11);
        assert_eq!(
            mgr.get_program(&id).unwrap().status,
            ProgramStatus::Expired
        );
    }

    // ----------------------------------------------------------------
    // test_get_recipient_rewards
    // ----------------------------------------------------------------

    #[test]
    fn test_get_recipient_rewards() {
        let mut mgr = default_manager();
        let id1 = mgr
            .create_program("Program A".to_string(), 10_000, 1_000, program_admin(), 1)
            .unwrap();
        let id2 = mgr
            .create_program("Program B".to_string(), 10_000, 1_000, program_admin(), 2)
            .unwrap();

        mgr.award_reward(&id1, contributor(0x01), 300, RewardReason::BugBounty, 3, &program_admin())
            .unwrap();
        mgr.award_reward(&id2, contributor(0x01), 700, RewardReason::Documentation, 4, &program_admin())
            .unwrap();
        // Different recipient
        mgr.award_reward(&id1, contributor(0x02), 100, RewardReason::ToolDevelopment, 5, &program_admin())
            .unwrap();

        let rewards = mgr.get_recipient_rewards(&contributor(0x01));
        assert_eq!(rewards.len(), 2);
        let total: Amount = rewards.iter().map(|(_, r)| r.amount).sum();
        assert_eq!(total, 1_000);
    }

    // ----------------------------------------------------------------
    // test_total_tracking
    // ----------------------------------------------------------------

    #[test]
    fn test_total_tracking() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        mgr.award_reward(&id, contributor(0x01), 1_000, RewardReason::BugBounty, 2, &program_admin())
            .unwrap();
        mgr.award_reward(&id, contributor(0x02), 2_000, RewardReason::FeatureDevelopment, 3, &program_admin())
            .unwrap();

        assert_eq!(mgr.get_total_rewarded(), 3_000);
        assert_eq!(mgr.get_program(&id).unwrap().spent, 3_000);
    }

    // ----------------------------------------------------------------
    // test_multiple_programs
    // ----------------------------------------------------------------

    #[test]
    fn test_multiple_programs() {
        let mut mgr = default_manager();

        let id1 = mgr
            .create_program("Alpha".to_string(), 5_000, 500, program_admin(), 1)
            .unwrap();
        let id2 = mgr
            .create_program("Beta".to_string(), 8_000, 800, program_admin(), 2)
            .unwrap();

        mgr.award_reward(&id1, contributor(0x01), 1_000, RewardReason::BugBounty, 3, &program_admin())
            .unwrap();
        mgr.award_reward(&id2, contributor(0x02), 2_000, RewardReason::SecurityAudit, 4, &program_admin())
            .unwrap();

        assert_eq!(mgr.total_programs, 2);
        assert_eq!(mgr.get_total_rewarded(), 3_000);
        assert_eq!(mgr.active_programs().len(), 2);
    }

    // ----------------------------------------------------------------
    // test_invalid_amount
    // ----------------------------------------------------------------

    #[test]
    fn test_invalid_amount() {
        let mut mgr = default_manager();

        // Zero budget
        let err = mgr
            .create_program("Zero Budget".to_string(), 0, 100, program_admin(), 1)
            .unwrap_err();
        assert_eq!(err, IncentiveError::InvalidAmount);

        // Zero reward
        let id = create_default_program(&mut mgr);
        let err = mgr
            .award_reward(&id, contributor(0x01), 0, RewardReason::BugBounty, 2, &program_admin())
            .unwrap_err();
        assert_eq!(err, IncentiveError::InvalidAmount);
    }

    // ----------------------------------------------------------------
    // test_program_exhausted_transitions
    // ----------------------------------------------------------------

    #[test]
    fn test_program_exhausted_transitions() {
        let mut mgr = default_manager();
        // Budget of exactly 500
        let id = mgr
            .create_program("Small".to_string(), 500, 1_000, program_admin(), 1)
            .unwrap();

        mgr.award_reward(&id, contributor(0x01), 500, RewardReason::BugBounty, 2, &program_admin())
            .unwrap();

        // Program should now be Exhausted
        assert_eq!(
            mgr.get_program(&id).unwrap().status,
            ProgramStatus::Exhausted
        );

        // Further awards should fail with ProgramNotActive
        let err = mgr
            .award_reward(&id, contributor(0x02), 1, RewardReason::BugBounty, 3, &program_admin())
            .unwrap_err();
        assert_eq!(err, IncentiveError::ProgramNotActive);
    }

    // ----------------------------------------------------------------
    // test_get_rewards_unknown_program
    // ----------------------------------------------------------------

    #[test]
    fn test_get_rewards_unknown_program() {
        let mgr = default_manager();
        let fake_id = Hash::from([0x99; 32]);
        assert!(mgr.get_rewards(&fake_id).is_empty());
    }

    // ----------------------------------------------------------------
    // test_custom_reward_reason
    // ----------------------------------------------------------------

    #[test]
    fn test_custom_reward_reason() {
        let mut mgr = default_manager();
        let id = create_default_program(&mut mgr);

        mgr.award_reward(
            &id,
            contributor(0x01),
            250,
            RewardReason::Custom("Hackathon winner".to_string()),
            2,
            &program_admin(),
        )
        .unwrap();

        let rewards = mgr.get_rewards(&id);
        assert_eq!(rewards.len(), 1);
        assert_eq!(
            rewards[0].reason,
            RewardReason::Custom("Hackathon winner".to_string())
        );
    }
}
