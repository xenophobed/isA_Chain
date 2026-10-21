use super::*;
use crate::incentives::types::{IncentiveError, ProgramStatus, RewardReason};
use crate::types::Hash;

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

#[test]
fn test_get_rewards_unknown_program() {
    let mgr = default_manager();
    let fake_id = Hash::from([0x99; 32]);
    assert!(mgr.get_rewards(&fake_id).is_empty());
}
