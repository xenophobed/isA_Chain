use super::*;
use crate::incentives::types::{IncentiveError, ProgramStatus, RewardReason};
use crate::types::Amount;

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
