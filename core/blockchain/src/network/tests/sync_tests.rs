use super::*;
use crate::network::types::SyncStatus;

#[test]
fn test_sync_manager_needs_sync() {
    let mut mgr = SyncManager::new(10);
    assert!(!mgr.needs_sync(), "no peers yet, should not need sync");

    mgr.update_peer_height(20);
    assert!(mgr.needs_sync());
}

#[test]
fn test_sync_manager_get_blocks_to_request() {
    let mut mgr = SyncManager::new(0);
    mgr.update_peer_height(100);

    let req = mgr.get_blocks_to_request(32);
    assert!(req.is_some());
    let (start, count) = req.unwrap();
    assert_eq!(start, 1, "should request from height 1");
    assert_eq!(count, 32);
}

#[test]
fn test_sync_manager_get_blocks_no_request_when_synced() {
    let mut mgr = SyncManager::new(50);
    mgr.update_peer_height(50);

    let req = mgr.get_blocks_to_request(16);
    assert!(req.is_none(), "already synced, nothing to request");
}

#[test]
fn test_sync_manager_mark_received() {
    let mut mgr = SyncManager::new(0);
    mgr.update_peer_height(5);

    // Request a batch and then mark each block received.
    mgr.get_blocks_to_request(5).unwrap();
    mgr.mark_received(1);
    mgr.mark_received(2);
    mgr.mark_received(3);
    mgr.mark_received(4);
    mgr.mark_received(5);

    assert_eq!(mgr.local_height, 5);
}

#[test]
fn test_sync_manager_synced() {
    let mut mgr = SyncManager::new(0);
    mgr.update_peer_height(3);

    assert!(!mgr.is_synced());

    mgr.update_local_height(3);
    assert!(mgr.is_synced());
    assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);
}

#[test]
fn test_sync_manager_status_transitions() {
    let mut mgr = SyncManager::new(10);
    // Before any peers: local == best → Synced.
    assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);

    mgr.update_peer_height(20);
    assert_eq!(
        *mgr.get_sync_status(),
        SyncStatus::Syncing {
            target_height: 20,
            current_height: 10,
        }
    );

    mgr.update_local_height(20);
    assert_eq!(*mgr.get_sync_status(), SyncStatus::Synced);
}
