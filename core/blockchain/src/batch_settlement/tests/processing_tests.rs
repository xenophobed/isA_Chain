use super::*;
use crate::batch_settlement::types::BatchStatus;

#[test]
fn test_process_batch() {
    let mut p = processor();
    let batch = p.create_batch(vec![settlement(1, 1_000, 1)], HEIGHT).unwrap();
    let batch_id = batch.id;

    let processed = p.process_batch(&batch_id).unwrap();
    assert_eq!(processed.status, BatchStatus::Completed);
    assert_eq!(p.get_total_batches(), 1);
    assert_eq!(p.get_total_settled(), 1_000);
}

#[test]
fn test_fee_calculation() {
    let p = processor(); // 250 bps = 2.5%

    assert_eq!(p.calculate_fee(1_000_000), 25_000);
    assert_eq!(p.calculate_fee(0), 0);
    // Small amount: 100 * 250 / 10_000 = 2 (floor)
    assert_eq!(p.calculate_fee(100), 2);
    // 1 * 250 / 10_000 = 0 (floor)
    assert_eq!(p.calculate_fee(1), 0);
}

#[test]
fn test_get_batches_at_height() {
    let mut p = processor();
    p.create_batch(vec![settlement(1, 100, 1)], 10).unwrap();
    p.create_batch(vec![settlement(2, 200, 1)], 10).unwrap();
    p.create_batch(vec![settlement(3, 300, 1)], 20).unwrap();

    let at_10 = p.get_batches_at_height(10);
    assert_eq!(at_10.len(), 2);

    let at_20 = p.get_batches_at_height(20);
    assert_eq!(at_20.len(), 1);

    let at_99 = p.get_batches_at_height(99);
    assert!(at_99.is_empty());
}

#[test]
fn test_total_tracking() {
    let mut p = processor();

    let b1 = p.create_batch(vec![settlement(1, 1_000, 1), settlement(2, 2_000, 1)], 1).unwrap();
    let b2 = p.create_batch(vec![settlement(3, 500, 1)], 2).unwrap();

    p.process_batch(&b1.id).unwrap();
    p.process_batch(&b2.id).unwrap();

    assert_eq!(p.get_total_settled(), 3_500);
    assert_eq!(p.get_total_batches(), 2);
}

#[test]
fn test_partially_completed() {
    let mut p = processor();
    let batch = p
        .create_batch(vec![settlement(1, 100, 1), settlement(2, 200, 1)], HEIGHT)
        .unwrap();
    let batch_id = batch.id;

    // Manually set PartiallyCompleted to confirm the enum round-trips
    let idx = p.batches.iter().position(|b| b.id == batch_id).unwrap();
    p.batches[idx].status = BatchStatus::PartiallyCompleted {
        succeeded: 1,
        failed: 1,
    };

    let b = p.get_batch(&batch_id).unwrap();
    assert!(matches!(
        b.status,
        BatchStatus::PartiallyCompleted { succeeded: 1, failed: 1 }
    ));
}

#[test]
fn test_multiple_batches() {
    let mut p = processor();

    // Create and process 5 separate single-channel batches
    for seed in 1u8..=5 {
        let batch = p
            .create_batch(vec![settlement(seed, 1_000, 1)], HEIGHT + seed as u64)
            .unwrap();
        p.process_batch(&batch.id).unwrap();
    }

    assert_eq!(p.get_total_batches(), 5);
    assert_eq!(p.get_total_settled(), 5_000);
    assert_eq!(p.settled_channels.len(), 5);

    // All 5 channel IDs should be marked settled
    for seed in 1u8..=5 {
        assert!(p.is_channel_settled(&channel_id(seed)));
    }
    // Channel 6 was never settled
    assert!(!p.is_channel_settled(&channel_id(6)));
}

#[test]
fn test_is_channel_settled() {
    let mut p = processor();
    let cid = channel_id(42);

    assert!(!p.is_channel_settled(&cid));

    let batch = p
        .create_batch(vec![(cid, sender(), receiver(), 500, 1)], HEIGHT)
        .unwrap();
    // Not yet settled until process_batch is called
    assert!(!p.is_channel_settled(&cid));

    p.process_batch(&batch.id).unwrap();
    assert!(p.is_channel_settled(&cid));
}
