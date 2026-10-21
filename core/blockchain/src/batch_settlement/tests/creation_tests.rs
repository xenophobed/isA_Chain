use super::*;
use crate::batch_settlement::types::{BatchSettlementError, BatchStatus};
use crate::types::constants::PROTOCOL_FEE_PERCENT;
use crate::types::Hash;

#[test]
fn test_create_batch() {
    let mut p = processor();
    let batch = p
        .create_batch(vec![settlement(1, 1_000, 1), settlement(2, 2_000, 2)], HEIGHT)
        .unwrap();

    assert_eq!(batch.settlements.len(), 2);
    assert_eq!(batch.total_amount, 3_000);
    assert_eq!(batch.height, HEIGHT);
    assert_eq!(batch.status, BatchStatus::Pending);
    assert_ne!(batch.id, Hash::ZERO);

    // Fee for 1_000 at 250 bps = 25; fee for 2_000 = 50
    assert_eq!(batch.total_fees, 75);
}

#[test]
fn test_empty_batch_fails() {
    let mut p = processor();
    let result = p.create_batch(vec![], HEIGHT);
    assert!(matches!(result, Err(BatchSettlementError::EmptyBatch)));
}

#[test]
fn test_batch_too_large() {
    let mut p = BatchProcessor::new(3, PROTOCOL_FEE_PERCENT);
    let settlements: Vec<_> = (1..=4).map(|i| settlement(i, 100, i as u64)).collect();
    let result = p.create_batch(settlements, HEIGHT);
    assert!(matches!(
        result,
        Err(BatchSettlementError::BatchTooLarge { max: 3, actual: 4 })
    ));
}

#[test]
fn test_duplicate_channel_fails() {
    let mut p = processor();

    // First batch: settle channel 1
    let batch = p.create_batch(vec![settlement(1, 1_000, 1)], HEIGHT).unwrap();
    p.process_batch(&batch.id).unwrap();

    // Channel 1 is now in settled_channels — second batch must fail
    let result = p.create_batch(vec![settlement(1, 500, 2)], HEIGHT + 1);
    assert!(matches!(result, Err(BatchSettlementError::AlreadySettled(_))));
}

#[test]
fn test_duplicate_channel_within_batch_fails() {
    let mut p = processor();

    // Two entries with the same channel_id in a single batch
    let cid = channel_id(1);
    let dup = vec![
        (cid, sender(), receiver(), 500, 1u64),
        (cid, sender(), receiver(), 500, 2u64),
    ];
    let result = p.create_batch(dup, HEIGHT);
    assert!(matches!(result, Err(BatchSettlementError::InvalidSettlement(_))));
}

#[test]
fn test_zero_amount_settlement_fails() {
    let mut p = processor();
    let result = p.create_batch(vec![settlement(1, 0, 1)], HEIGHT);
    assert!(matches!(result, Err(BatchSettlementError::InvalidSettlement(_))));
}
