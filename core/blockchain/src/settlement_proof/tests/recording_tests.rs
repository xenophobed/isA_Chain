use super::*;
use crate::settlement_proof::types::{ProofStatus, SettlementProofError};

#[test]
fn test_record_proof() {
    let mut store = make_store();
    let proof_id = record_default(&mut store);

    assert_ne!(proof_id, Hash::ZERO);
    let rec = store.get_proof(&proof_id).unwrap();
    assert_eq!(rec.settlement_id, settlement_id());
    assert_eq!(rec.tx_hash, tx_hash());
    assert_eq!(rec.billing_event_id, "billing-event-001");
    assert_eq!(rec.user, user());
    assert_eq!(rec.provider, provider());
    assert_eq!(rec.amount, 1_000_000);
    assert_eq!(rec.fee, 25_000);
    assert_eq!(rec.block_height, 10);
    assert_eq!(rec.confirmations, 0);
    assert_eq!(rec.status, ProofStatus::Pending);
}

#[test]
fn test_duplicate_proof_fails() {
    let mut store = make_store();
    record_default(&mut store);

    // Same settlement_id should fail.
    let result = store.record_proof(
        settlement_id(),
        Hash::hash_data(b"tx-hash-2"),
        "billing-event-002".to_string(),
        user(),
        provider(),
        500_000,
        12_500,
        ServiceType::ToolExecution,
        20,
        2_000_000,
    );

    assert!(matches!(
        result,
        Err(SettlementProofError::DuplicateProof(_))
    ));
}

#[test]
fn test_duplicate_billing_event() {
    let mut store = make_store();
    record_default(&mut store);

    // Different settlement but same billing_event_id should fail.
    let result = store.record_proof(
        Hash::hash_data(b"settlement-2"),
        Hash::hash_data(b"tx-hash-2"),
        "billing-event-001".to_string(), // duplicate
        user(),
        provider(),
        500_000,
        12_500,
        ServiceType::ToolExecution,
        20,
        2_000_000,
    );

    assert!(matches!(
        result,
        Err(SettlementProofError::DuplicateBillingEvent(_))
    ));
}

#[test]
fn test_get_by_settlement() {
    let mut store = make_store();
    let proof_id = record_default(&mut store);

    let rec = store.get_proof_by_settlement(&settlement_id()).unwrap();
    assert_eq!(rec.proof_id, proof_id);
}

#[test]
fn test_get_by_billing_event() {
    let mut store = make_store();
    let proof_id = record_default(&mut store);

    let rec = store
        .get_proof_by_billing_event("billing-event-001")
        .unwrap();
    assert_eq!(rec.proof_id, proof_id);
}

#[test]
fn test_get_user_proofs() {
    let mut store = make_store();

    // Two proofs for the same user, different settlements/billing events.
    store
        .record_proof(
            Hash::hash_data(b"s1"),
            Hash::hash_data(b"tx1"),
            "evt-1".to_string(),
            user(),
            provider(),
            1_000,
            25,
            ServiceType::ModelInference,
            1,
            1_000,
        )
        .unwrap();

    store
        .record_proof(
            Hash::hash_data(b"s2"),
            Hash::hash_data(b"tx2"),
            "evt-2".to_string(),
            user(),
            provider(),
            2_000,
            50,
            ServiceType::ToolExecution,
            2,
            2_000,
        )
        .unwrap();

    // Proof from a different user — should NOT appear.
    let other_user = Address::from([0xCC; 20]);
    store
        .record_proof(
            Hash::hash_data(b"s3"),
            Hash::hash_data(b"tx3"),
            "evt-3".to_string(),
            other_user,
            provider(),
            3_000,
            75,
            ServiceType::Storage,
            3,
            3_000,
        )
        .unwrap();

    let proofs = store.get_user_proofs(&user());
    assert_eq!(proofs.len(), 2);
    for p in &proofs {
        assert_eq!(p.user, user());
    }
}
