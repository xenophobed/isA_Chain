use crate::finality::types::FinalityVote;
use crate::types::{Address, BlockHeight, Hash, Signature};

pub fn make_address(byte: u8) -> Address {
    let mut bytes = [0u8; 20];
    bytes[0] = byte;
    Address::from(bytes)
}

pub fn make_hash(byte: u8) -> Hash {
    let mut bytes = [0u8; 32];
    bytes[0] = byte;
    Hash::from(bytes)
}

pub fn dummy_sig() -> Signature {
    Signature::new([1u8; 32], [2u8; 32], 27)
}

pub fn make_vote(
    voter: Address,
    block_hash: Hash,
    height: BlockHeight,
    round: u32,
) -> FinalityVote {
    FinalityVote {
        voter,
        block_hash,
        height,
        round,
        signature: dummy_sig(),
    }
}
