use crate::compute_market::types::*;
use crate::types::*;

impl ComputeMarketState {
    /// Open a dispute on a job
    pub fn open_dispute(
        &mut self,
        dispute_id: Hash,
        job_id: Hash,
        initiator: Address,
        dispute_type: DisputeType,
        evidence_hash: Hash,
        created_at: Timestamp,
    ) -> Result<(), ComputeMarketError> {
        let job = self
            .jobs
            .get_mut(&job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Verify initiator is user or provider
        let is_user = job.user == initiator;
        let is_provider = job.provider == Some(initiator);
        if !is_user && !is_provider {
            return Err(ComputeMarketError::NotAuthorized);
        }

        // Can only dispute running or completed jobs
        if !matches!(job.status, JobStatus::Running | JobStatus::Completed) {
            return Err(ComputeMarketError::InvalidJobState);
        }

        // Check dispute window for completed jobs
        if job.status == JobStatus::Completed {
            if let Some(ended_at) = job.ended_at {
                if created_at > ended_at + constants::DISPUTE_WINDOW_SECS * 1000 {
                    return Err(ComputeMarketError::DisputeWindowExpired);
                }
            }
        }

        let dispute = ComputeDispute {
            dispute_id,
            job_id,
            initiator,
            dispute_type,
            evidence_hash,
            created_at,
            deadline: created_at + constants::DISPUTE_WINDOW_SECS * 1000,
            resolution: None,
        };

        job.status = JobStatus::Disputed;
        self.disputes.insert(dispute_id, dispute);
        self.stats.total_jobs_disputed += 1;

        Ok(())
    }

    /// Resolve a dispute
    #[allow(clippy::too_many_arguments)]
    pub fn resolve_dispute(
        &mut self,
        dispute_id: Hash,
        winner: Address,
        user_refund: Amount,
        provider_payment: Amount,
        slash_amount: Amount,
        resolved_by: Address,
        resolved_at: Timestamp,
    ) -> Result<DisputeResolution, ComputeMarketError> {
        let dispute = self
            .disputes
            .get_mut(&dispute_id)
            .ok_or(ComputeMarketError::DisputeNotFound)?;

        if dispute.resolution.is_some() {
            return Err(ComputeMarketError::DisputeAlreadyResolved);
        }

        let job = self
            .jobs
            .get_mut(&dispute.job_id)
            .ok_or(ComputeMarketError::JobNotFound)?;

        // Apply slash to provider if any
        if slash_amount > 0 {
            if let Some(provider_addr) = job.provider {
                if let Some(provider) = self.providers.get_mut(&provider_addr) {
                    let actual_slash = slash_amount.min(provider.stake);
                    provider.stake = provider.stake.saturating_sub(actual_slash);
                    self.total_provider_stake =
                        self.total_provider_stake.saturating_sub(actual_slash);
                    self.treasury_balance += actual_slash; // Slashed funds go to treasury

                    // Reputation hit
                    provider.reputation = provider.reputation.saturating_sub(500);
                    provider.jobs_failed += 1;
                }
            }
        }

        let resolution = DisputeResolution {
            winner,
            user_refund,
            provider_payment,
            slash_amount,
            resolved_by,
            resolved_at,
        };

        dispute.resolution = Some(resolution.clone());

        // Update job status based on outcome
        if user_refund > provider_payment {
            job.status = JobStatus::Failed;
            self.stats.total_jobs_failed += 1;
        } else {
            job.status = JobStatus::Completed;
            self.stats.total_jobs_completed += 1;
        }

        // Release escrow
        self.total_escrow = self.total_escrow.saturating_sub(job.escrow_amount);

        Ok(resolution)
    }
}
