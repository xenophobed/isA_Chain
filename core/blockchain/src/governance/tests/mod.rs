use crate::governance::governor::SubnetGovernor;
use crate::governance::types::ProposalAction;
use crate::subnet::SubnetId;
use crate::types::{Address, Amount, BlockHeight, Hash};

mod proposal_voting_tests;
mod tally_execution_tests;

pub(crate) const ISA: Amount = 1_000_000_000_000_000_000; // 1 ISA in wei

pub(crate) fn isa(n: u128) -> Amount {
    n * ISA
}

pub(crate) fn addr(byte: u8) -> Address {
    Address::from([byte; 20])
}

/// Default governor: 1 000-block voting period, 10 000 ISA min stake.
pub(crate) fn governor() -> SubnetGovernor {
    SubnetGovernor::new(1_000, isa(10_000))
}

pub(crate) fn make_proposal(
    gov: &mut SubnetGovernor,
    proposer: Address,
    height: BlockHeight,
) -> Hash {
    gov.create_proposal(
        SubnetId::Model,
        proposer,
        isa(10_000),
        "Test proposal".to_string(),
        "A test proposal description".to_string(),
        ProposalAction::ChangeFeeRate(300),
        height,
    )
    .unwrap()
}
