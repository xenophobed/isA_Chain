use super::*;
use crate::types::Hash;

pub(crate) fn genesis() -> Hash {
    Hash::hash_data(b"genesis")
}

pub(crate) fn head() -> Hash {
    Hash::hash_data(b"head_block")
}

mod message_tests;
mod sync_tests;
