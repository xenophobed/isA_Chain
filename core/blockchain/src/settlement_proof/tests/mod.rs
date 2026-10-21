use crate::settlement::ServiceType;
use crate::settlement_proof::store::ProofStore;
use crate::types::{Address, Hash};

mod lifecycle_tests;
mod recording_tests;

// ---- Helpers -------------------------------------------------------

pub(crate) fn user() -> Address {
    Address::from([0xAA; 20])
}

pub(crate) fn provider() -> Address {
    Address::from([0xBB; 20])
}

pub(crate) fn settlement_id() -> Hash {
    Hash::hash_data(b"settlement-1")
}

pub(crate) fn tx_hash() -> Hash {
    Hash::hash_data(b"tx-hash-1")
}

pub(crate) fn make_store() -> ProofStore {
    ProofStore::new(6, 100)
}

pub(crate) fn record_default(store: &mut ProofStore) -> Hash {
    store
        .record_proof(
            settlement_id(),
            tx_hash(),
            "billing-event-001".to_string(),
            user(),
            provider(),
            1_000_000,
            25_000,
            ServiceType::ModelInference,
            10,
            1_000_000,
        )
        .unwrap()
}
