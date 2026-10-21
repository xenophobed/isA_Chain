use super::*;
use crate::governance::types::{GovernanceError, ProposalAction, ProposalStatus};

#[test]
fn test_tally_passed() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    // Total subnet stake = 100 000 ISA.
    // Quorum needed: 30 % = 30 000 ISA.
    // Threshold: 60 % of participating votes.
    let total_stake = isa(100_000);

    // 40 000 ISA for, 5 000 ISA against → total 45 000 (> 30 000 quorum).
    // votes_for / total = 40 / 45 ≈ 88.9 % > 60 %.
    gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
    gov.vote(&id, addr(0x03), isa(5_000), false, 500).unwrap();

    let status = gov.tally(&id, total_stake, 1_001).unwrap();
    assert_eq!(status, ProposalStatus::Passed);
    assert_eq!(gov.get_proposal(&id).unwrap().status, ProposalStatus::Passed);
}

#[test]
fn test_tally_rejected() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    let total_stake = isa(100_000);

    // 40 000 for, 40 000 against → 50 % < 60 % threshold.
    gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
    gov.vote(&id, addr(0x03), isa(40_000), false, 500).unwrap();

    let status = gov.tally(&id, total_stake, 1_001).unwrap();
    assert_eq!(status, ProposalStatus::Rejected);
}

#[test]
fn test_tally_no_quorum() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    let total_stake = isa(100_000);

    // Only 5 000 ISA voted (5 % of 100 000) — well below the 30 % quorum.
    gov.vote(&id, addr(0x02), isa(5_000), true, 500).unwrap();

    let status = gov.tally(&id, total_stake, 1_001).unwrap();
    assert_eq!(status, ProposalStatus::Rejected);
}

#[test]
fn test_execute() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    // Sufficient votes to pass.
    gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
    gov.tally(&id, isa(100_000), 1_001).unwrap();

    let action = gov.execute(&id).unwrap();
    assert_eq!(action, ProposalAction::ChangeFeeRate(300));
    assert_eq!(
        gov.get_proposal(&id).unwrap().status,
        ProposalStatus::Executed
    );
}

#[test]
fn test_execute_not_passed_fails() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    // Don't vote at all — proposal is still Active.
    let err = gov.execute(&id).unwrap_err();
    assert_eq!(err, GovernanceError::ProposalNotPassed);
}

#[test]
fn test_execute_already_executed_fails() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 0);

    gov.vote(&id, addr(0x02), isa(40_000), true, 500).unwrap();
    gov.tally(&id, isa(100_000), 1_001).unwrap();
    gov.execute(&id).unwrap();

    let err = gov.execute(&id).unwrap_err();
    assert_eq!(err, GovernanceError::AlreadyExecuted);
}

#[test]
fn test_expire_proposals() {
    let mut gov = governor();
    let id = make_proposal(&mut gov, addr(0x01), 0);

    // Before expiry — still Active.
    gov.expire_proposals(500);
    assert_eq!(
        gov.get_proposal(&id).unwrap().status,
        ProposalStatus::Active
    );

    // After voting_ends_at (1 000) — Expired.
    gov.expire_proposals(1_001);
    assert_eq!(
        gov.get_proposal(&id).unwrap().status,
        ProposalStatus::Expired
    );
}

#[test]
fn test_tally_before_end_fails() {
    let mut gov = governor();
    let id = make_proposal(&mut gov, addr(0x01), 0);

    // Voting ends at 1 000; try to tally at 999.
    let err = gov.tally(&id, isa(100_000), 999).unwrap_err();
    assert_eq!(err, GovernanceError::VotingNotEnded);
}

#[test]
fn test_proposal_not_found() {
    let mut gov = governor();
    let fake_id = Hash::from([0xDE; 32]);

    assert!(gov.get_proposal(&fake_id).is_none());
    assert_eq!(
        gov.vote(&fake_id, addr(0x01), isa(100), true, 1).unwrap_err(),
        GovernanceError::ProposalNotFound(fake_id)
    );
    assert_eq!(
        gov.tally(&fake_id, isa(1_000), 2_000).unwrap_err(),
        GovernanceError::ProposalNotFound(fake_id)
    );
    assert_eq!(
        gov.execute(&fake_id).unwrap_err(),
        GovernanceError::ProposalNotFound(fake_id)
    );
}
