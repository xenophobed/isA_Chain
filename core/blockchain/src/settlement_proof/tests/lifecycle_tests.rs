use super::*;
use crate::settlement_proof::types::{ProofStatus, SettlementProofError};

#[test]
fn test_update_confirmations_pending() {
    let mut store = make_store(); // needs 6 to confirm
    let proof_id = record_default(&mut store); // block_height = 10

    // current_height = 14 → confirmations = 4 (< 6) → still Pending
    let status = store.update_confirmations(&proof_id, 14).unwrap();
    assert_eq!(status, ProofStatus::Pending);

    let rec = store.get_proof(&proof_id).unwrap();
    assert_eq!(rec.confirmations, 4);
}

#[test]
fn test_update_confirmations_confirmed() {
    let mut store = make_store(); // confirmations_required = 6
    let proof_id = record_default(&mut store); // block_height = 10

    // current_height = 16 → confirmations = 6 → Confirmed
    let status = store.update_confirmations(&proof_id, 16).unwrap();
    assert_eq!(status, ProofStatus::Confirmed);
}

#[test]
fn test_update_confirmations_finalized() {
    let mut store = make_store(); // finalization_depth = 100
    let proof_id = record_default(&mut store); // block_height = 10

    // current_height = 110 → confirmations = 100 → Finalized
    let status = store.update_confirmations(&proof_id, 110).unwrap();
    assert_eq!(status, ProofStatus::Finalized);

    let rec = store.get_proof(&proof_id).unwrap();
    assert_eq!(rec.confirmations, 100);
}

#[test]
fn test_update_confirmations_not_found() {
    let mut store = make_store();
    let unknown = Hash::hash_data(b"nonexistent");
    let result = store.update_confirmations(&unknown, 100);
    assert!(matches!(
        result,
        Err(SettlementProofError::ProofNotFound(_))
    ));
}

#[test]
fn test_verify_proof() {
    let mut store = make_store();
    let proof_id = record_default(&mut store); // block_height = 10

    // Pending → not verified yet
    assert_eq!(store.verify_proof(&proof_id).unwrap(), false);

    // Advance to Confirmed
    store.update_confirmations(&proof_id, 16).unwrap();
    assert_eq!(store.verify_proof(&proof_id).unwrap(), true);

    // Advance to Finalized — still verified
    store.update_confirmations(&proof_id, 110).unwrap();
    assert_eq!(store.verify_proof(&proof_id).unwrap(), true);
}

#[test]
fn test_verify_proof_not_found() {
    let store = make_store();
    let unknown = Hash::hash_data(b"ghost");
    let result = store.verify_proof(&unknown);
    assert!(matches!(
        result,
        Err(SettlementProofError::ProofNotFound(_))
    ));
}

#[test]
fn test_verify_failed_proof() {
    let mut store = make_store();
    let proof_id = record_default(&mut store);

    // Manually set to Failed.
    store.proofs.get_mut(&proof_id).unwrap().status =
        ProofStatus::Failed("reverted".to_string());

    let result = store.verify_proof(&proof_id);
    assert!(matches!(result, Err(SettlementProofError::InvalidProof)));
}

#[test]
fn test_pending_confirmed_counts() {
    let mut store = make_store(); // confirmations_required = 6

    // Record two proofs at different block heights.
    let p1 = store
        .record_proof(
            Hash::hash_data(b"s1"),
            Hash::hash_data(b"tx1"),
            "evt-1".to_string(),
            user(),
            provider(),
            1_000,
            25,
            ServiceType::ModelInference,
            10,
            1_000,
        )
        .unwrap();

    let p2 = store
        .record_proof(
            Hash::hash_data(b"s2"),
            Hash::hash_data(b"tx2"),
            "evt-2".to_string(),
            user(),
            provider(),
            2_000,
            50,
            ServiceType::ToolExecution,
            10,
            2_000,
        )
        .unwrap();

    assert_eq!(store.get_pending_count(), 2);
    assert_eq!(store.get_confirmed_count(), 0);

    // Confirm p1 only.
    store.update_confirmations(&p1, 16).unwrap();

    assert_eq!(store.get_pending_count(), 1);
    assert_eq!(store.get_confirmed_count(), 1);

    // Finalize p1 — confirmed count should drop.
    store.update_confirmations(&p1, 110).unwrap();

    assert_eq!(store.get_pending_count(), 1);
    assert_eq!(store.get_confirmed_count(), 0);

    // Confirm p2.
    store.update_confirmations(&p2, 16).unwrap();
    assert_eq!(store.get_confirmed_count(), 1);
}
