use super::*;
use crate::governance::types::{GovernanceError, ProposalAction, ProposalStatus};

#[test]
fn test_create_proposal() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let id = make_proposal(&mut gov, proposer, 100);

    let p = gov.get_proposal(&id).unwrap();
    assert_eq!(p.subnet_id, SubnetId::Model);
    assert_eq!(p.proposer, proposer);
    assert_eq!(p.status, ProposalStatus::Active);
    assert_eq!(p.created_at, 100);
    assert_eq!(p.voting_ends_at, 1_100); // 100 + 1_000
    assert_eq!(p.quorum_bps, 3000);
    assert_eq!(p.threshold_bps, 6000);
    assert_eq!(p.votes_for, 0);
    assert_eq!(p.votes_against, 0);

    // Indexed by subnet.
    assert_eq!(gov.get_subnet_proposals(&SubnetId::Model).len(), 1);
}

#[test]
fn test_vote() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let voter_a = addr(0x02);
    let voter_b = addr(0x03);
    let id = make_proposal(&mut gov, proposer, 0);

    gov.vote(&id, voter_a, isa(5_000), true, 500).unwrap();
    gov.vote(&id, voter_b, isa(2_000), false, 500).unwrap();

    let p = gov.get_proposal(&id).unwrap();
    assert_eq!(p.votes_for, isa(5_000));
    assert_eq!(p.votes_against, isa(2_000));
    assert!(p.voters.contains(&voter_a));
    assert!(p.voters.contains(&voter_b));
}

#[test]
fn test_double_vote_fails() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let voter = addr(0x02);
    let id = make_proposal(&mut gov, proposer, 0);

    gov.vote(&id, voter, isa(1_000), true, 10).unwrap();
    let err = gov.vote(&id, voter, isa(1_000), true, 20).unwrap_err();
    assert_eq!(err, GovernanceError::AlreadyVoted);
}

#[test]
fn test_vote_after_end_fails() {
    let mut gov = governor();
    let proposer = addr(0x01);
    let voter = addr(0x02);
    // Voting ends at block 1 000.
    let id = make_proposal(&mut gov, proposer, 0);

    // Try voting at block 1 001.
    let err = gov
        .vote(&id, voter, isa(1_000), true, 1_001)
        .unwrap_err();
    assert_eq!(err, GovernanceError::VotingEnded);
}

#[test]
fn test_vote_zero_stake_fails() {
    let mut gov = governor();
    let id = make_proposal(&mut gov, addr(0x01), 0);

    let err = gov
        .vote(&id, addr(0x02), 0, true, 100)
        .unwrap_err();
    assert_eq!(err, GovernanceError::NotStaker(addr(0x02)));
}

#[test]
fn test_insufficient_stake_to_propose() {
    let mut gov = governor();
    let err = gov
        .create_proposal(
            SubnetId::Tools,
            addr(0x01),
            isa(9_999), // below the 10 000 ISA minimum
            "Low stake proposal".to_string(),
            "Should fail".to_string(),
            ProposalAction::PauseSubnet,
            1,
        )
        .unwrap_err();
    assert_eq!(err, GovernanceError::InsufficientStake);
}

#[test]
fn test_get_active_proposals() {
    let mut gov = governor();
    let id1 = make_proposal(&mut gov, addr(0x01), 0);   // ends at 1 000
    let id2 = make_proposal(&mut gov, addr(0x02), 500); // ends at 1 500

    // At block 1 001: id1 voting period has ended, id2 is still active.
    let active = gov.get_active_proposals(&SubnetId::Model, 1_001);
    let active_ids: Vec<Hash> = active.iter().map(|p| p.id).collect();
    assert!(!active_ids.contains(&id1));
    assert!(active_ids.contains(&id2));
}
