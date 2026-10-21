use super::types::{IncentiveError, IncentiveProgram, ProgramStatus, Reward, RewardReason};
use crate::types::{Address, Amount, BlockHeight, Hash};
use std::collections::HashMap;

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
