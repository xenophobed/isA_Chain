use crate::types::*;
use crate::account::Account;
use std::collections::{HashMap, HashSet};

/// World state manager — single source of truth for account state.
///
/// Tracks which accounts have been modified since the last state root
/// computation so that `compute_state_root` only needs to re-hash dirty
/// accounts rather than the entire set.
pub struct WorldState {
    /// Account states keyed by address
    accounts: HashMap<Address, Account>,

    /// Most-recently computed (or zero) state root
    root: Hash,

    /// Block height associated with this state snapshot
    height: BlockHeight,

    /// Addresses modified since the last call to `compute_state_root`
    dirty: HashSet<Address>,
}

impl WorldState {
    /// Create an empty world state at height 0.
    pub fn new() -> Self {
        WorldState {
            accounts: HashMap::new(),
            root: Hash::ZERO,
            height: 0,
            dirty: HashSet::new(),
        }
    }

    // ─── Read accessors ───────────────────────────────────────────────────────

    /// Return a reference to the account at `address`, or `None` if it does
    /// not exist.
    pub fn get_account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    /// Return a mutable reference to the account at `address`, or `None`.
    ///
    /// Marks the account dirty so the state root will be recomputed on the
    /// next call to `compute_state_root`.
    pub fn get_account_mut(&mut self, address: &Address) -> Option<&mut Account> {
        if self.accounts.contains_key(address) {
            self.dirty.insert(*address);
            self.accounts.get_mut(address)
        } else {
            None
        }
    }

    /// Return the balance of `address`, or 0 if the account does not exist.
    pub fn get_balance(&self, address: &Address) -> Amount {
        self.accounts
            .get(address)
            .map(|a| a.balance)
            .unwrap_or(0)
    }

    /// Return the most-recently computed state root.
    ///
    /// This value is only up-to-date after calling `compute_state_root`.
    pub fn get_state_root(&self) -> Hash {
        self.root
    }

    /// Return the total number of accounts tracked in this state.
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }

    // ─── Write accessors ─────────────────────────────────────────────────────

    /// Insert or replace the account at `address`.
    ///
    /// Marks the account dirty.
    pub fn set_account(&mut self, address: Address, account: Account) {
        self.accounts.insert(address, account);
        self.dirty.insert(address);
    }

    /// Return a mutable reference to the account at `address`, creating a
    /// zero-balance external account if one does not yet exist.
    ///
    /// Marks the account dirty.
    pub fn get_or_create_account(&mut self, address: Address) -> &mut Account {
        self.accounts.entry(address).or_insert_with(|| Account::new_external(0));
        self.dirty.insert(address);
        self.accounts.get_mut(&address).expect("just inserted")
    }

    /// Set the block height associated with this state.
    pub fn set_height(&mut self, height: BlockHeight) {
        self.height = height;
    }

    // ─── State root ──────────────────────────────────────────────────────────

    /// Recompute and store the state root from all accounts, then clear the
    /// dirty set.
    ///
    /// The root is the Merkle root of `(address_bytes || account_hash)` for
    /// every account, sorted by address for determinism.
    pub fn compute_state_root(&mut self) -> Hash {
        // Collect all (address, account) pairs, sorted by address bytes for
        // a deterministic ordering regardless of HashMap iteration order.
        let mut pairs: Vec<(&Address, &Account)> = self.accounts.iter().collect();
        pairs.sort_by_key(|(addr, _)| addr.as_ref().to_vec());

        // Build one leaf hash per account: Hash(address_bytes || account_hash_bytes)
        let leaf_hashes: Vec<Hash> = pairs
            .iter()
            .map(|(addr, acct)| {
                let mut data = Vec::with_capacity(20 + 32);
                data.extend_from_slice(addr.as_ref());
                data.extend_from_slice(acct.hash().as_ref());
                Hash::hash_data(&data)
            })
            .collect();

        self.root = Hash::merkle_root(&leaf_hashes);
        self.dirty.clear();
        self.root
    }
}

impl Default for WorldState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(byte: u8) -> Address {
        Address::from([byte; 20])
    }

    #[test]
    fn test_new_world_state() {
        let ws = WorldState::new();
        assert_eq!(ws.account_count(), 0);
        assert_eq!(ws.get_state_root(), Hash::ZERO);
    }

    #[test]
    fn test_set_and_get_account() {
        let mut ws = WorldState::new();
        let a = addr(1);
        let account = Account::new_external(500);

        ws.set_account(a, account.clone());

        let fetched = ws.get_account(&a).expect("account should exist");
        assert_eq!(fetched.balance, 500);
        assert_eq!(ws.account_count(), 1);
    }

    #[test]
    fn test_get_balance() {
        let mut ws = WorldState::new();
        let a = addr(2);

        // Non-existent account returns 0
        assert_eq!(ws.get_balance(&a), 0);

        ws.set_account(a, Account::new_external(1234));
        assert_eq!(ws.get_balance(&a), 1234);
    }

    #[test]
    fn test_get_or_create_account() {
        let mut ws = WorldState::new();
        let a = addr(3);

        // First call creates an account with zero balance
        {
            let acct = ws.get_or_create_account(a);
            assert_eq!(acct.balance, 0);
            acct.balance = 42;
        }

        // Second call returns the existing (mutated) account
        {
            let acct = ws.get_or_create_account(a);
            assert_eq!(acct.balance, 42);
        }

        assert_eq!(ws.account_count(), 1);
    }

    #[test]
    fn test_compute_state_root_deterministic() {
        // Two world states with the same accounts should produce the same root.
        let a = addr(4);
        let b = addr(5);

        let mut ws1 = WorldState::new();
        ws1.set_account(a, Account::new_external(100));
        ws1.set_account(b, Account::new_external(200));
        let root1 = ws1.compute_state_root();

        let mut ws2 = WorldState::new();
        // Insert in the opposite order — root must still match.
        ws2.set_account(b, Account::new_external(200));
        ws2.set_account(a, Account::new_external(100));
        let root2 = ws2.compute_state_root();

        assert_eq!(root1, root2);
        assert_ne!(root1, Hash::ZERO);
    }

    #[test]
    fn test_state_root_changes_with_account() {
        let mut ws = WorldState::new();
        let a = addr(6);

        ws.set_account(a, Account::new_external(100));
        let root_before = ws.compute_state_root();

        // Mutate the account balance
        ws.get_or_create_account(a).balance = 999;
        let root_after = ws.compute_state_root();

        assert_ne!(root_before, root_after);
    }

    #[test]
    fn test_dirty_tracking() {
        let mut ws = WorldState::new();
        let a = addr(7);
        let b = addr(8);

        ws.set_account(a, Account::new_external(10));
        ws.set_account(b, Account::new_external(20));

        // Both addresses should be dirty before root computation
        assert!(ws.dirty.contains(&a));
        assert!(ws.dirty.contains(&b));

        ws.compute_state_root();

        // Dirty set should be cleared after computation
        assert!(ws.dirty.is_empty());

        // Mutating via get_account_mut marks dirty again
        ws.get_account_mut(&a).unwrap().balance = 999;
        assert!(ws.dirty.contains(&a));
        assert!(!ws.dirty.contains(&b));
    }

    #[test]
    fn test_account_count() {
        let mut ws = WorldState::new();

        assert_eq!(ws.account_count(), 0);

        ws.set_account(addr(9), Account::new_external(1));
        assert_eq!(ws.account_count(), 1);

        ws.set_account(addr(10), Account::new_external(2));
        assert_eq!(ws.account_count(), 2);

        // Overwriting an existing address must not change the count
        ws.set_account(addr(9), Account::new_external(999));
        assert_eq!(ws.account_count(), 2);
    }
}
