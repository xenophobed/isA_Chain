use crate::types::*;
use crate::transaction::Transaction;
use crate::error::*;
use std::collections::HashMap;

/// Transaction mempool
pub struct Mempool {
    /// Pending transactions by hash
    transactions: HashMap<Hash, Transaction>,
    
    /// Transactions by sender address
    by_sender: HashMap<Address, Vec<Hash>>,
    
    /// Maximum mempool size
    max_size: usize,
}

impl Mempool {
    pub fn new(max_size: usize) -> Self {
        Mempool {
            transactions: HashMap::new(),
            by_sender: HashMap::new(),
            max_size,
        }
    }
    
    pub fn add_transaction(&mut self, tx: Transaction) -> Result<(), MempoolError> {
        let hash = tx.hash();
        let sender = tx.from;
        
        if self.transactions.contains_key(&hash) {
            return Err(MempoolError::TransactionExists { hash });
        }
        
        if self.transactions.len() >= self.max_size {
            return Err(MempoolError::MempoolFull);
        }

        // Basic sanity: gas_limit must be non-zero
        if tx.gas_limit == 0 {
            return Err(MempoolError::GasPriceTooLow {
                minimum: 1,
                actual: 0,
            });
        }

        self.transactions.insert(hash, tx);
        self.by_sender.entry(sender).or_default().push(hash);
        
        Ok(())
    }
    
    pub fn get_transaction(&self, hash: &Hash) -> Option<&Transaction> {
        self.transactions.get(hash)
    }
    
    pub fn remove_transaction(&mut self, hash: &Hash) -> Option<Transaction> {
        if let Some(tx) = self.transactions.remove(hash) {
            // Remove from sender index
            if let Some(sender_txs) = self.by_sender.get_mut(&tx.from) {
                sender_txs.retain(|h| h != hash);
                if sender_txs.is_empty() {
                    self.by_sender.remove(&tx.from);
                }
            }
            Some(tx)
        } else {
            None
        }
    }
    
    pub fn len(&self) -> usize {
        self.transactions.len()
    }
    
    pub fn is_empty(&self) -> bool {
        self.transactions.is_empty()
    }

    /// Returns the current number of transactions in the mempool
    pub fn size(&self) -> usize {
        self.transactions.len()
    }

    /// Check whether a transaction with the given hash is present
    pub fn contains(&self, hash: &Hash) -> bool {
        self.transactions.contains_key(hash)
    }

    /// Get pending transactions for block building
    /// Returns transactions ordered by gas price and nonce
    pub fn get_pending_transactions(&self, max_count: usize) -> Vec<Transaction> {
        let mut txs: Vec<_> = self.transactions.values().cloned().collect();

        // Sort by gas price (descending) then nonce (ascending)
        txs.sort_by(|a, b| {
            match b.gas_price.cmp(&a.gas_price) {
                std::cmp::Ordering::Equal => a.nonce.cmp(&b.nonce),
                other => other,
            }
        });

        txs.into_iter().take(max_count).collect()
    }

    /// Get all pending transactions
    pub fn get_all_transactions(&self) -> Vec<Transaction> {
        self.transactions.values().cloned().collect()
    }

    /// Clear all transactions
    pub fn clear(&mut self) {
        self.transactions.clear();
        self.by_sender.clear();
    }

    /// Remove multiple transactions
    pub fn remove_transactions(&mut self, hashes: &[Hash]) {
        for hash in hashes {
            self.remove_transaction(hash);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transaction::{Transaction, TransactionData};
    use crate::types::constants;

    /// Build a minimal Transfer transaction for testing.
    fn make_tx(from: u8, to: u8, nonce: u64, gas_limit: Gas, gas_price: GasPrice) -> Transaction {
        Transaction::new(
            Address::from([from; 20]),
            nonce,
            TransactionData::Transfer {
                to: Address::from([to; 20]),
                amount: 1_000,
                data: vec![],
            },
            gas_limit,
            gas_price,
            constants::MAIN_CHAIN_ID,
        )
    }

    #[test]
    fn test_add_transaction() {
        let mut mp = Mempool::new(10);
        let tx = make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE);
        assert!(mp.add_transaction(tx).is_ok());
        assert_eq!(mp.size(), 1);
    }

    #[test]
    fn test_add_duplicate_rejected() {
        let mut mp = Mempool::new(10);
        let tx = make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE);
        let tx_clone = tx.clone();

        mp.add_transaction(tx).unwrap();
        let result = mp.add_transaction(tx_clone);

        assert!(matches!(result, Err(MempoolError::TransactionExists { .. })));
        assert_eq!(mp.size(), 1);
    }

    #[test]
    fn test_mempool_capacity() {
        let mut mp = Mempool::new(2);
        mp.add_transaction(make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE)).unwrap();
        mp.add_transaction(make_tx(1, 2, 1, 21_000, constants::BASE_GAS_PRICE)).unwrap();

        // Third transaction must fail
        let result = mp.add_transaction(make_tx(1, 2, 2, 21_000, constants::BASE_GAS_PRICE));
        assert!(matches!(result, Err(MempoolError::MempoolFull)));
        assert_eq!(mp.size(), 2);
    }

    #[test]
    fn test_gas_limit_zero_rejected() {
        let mut mp = Mempool::new(10);
        let tx = make_tx(1, 2, 0, 0, constants::BASE_GAS_PRICE);
        let result = mp.add_transaction(tx);
        // Should be rejected due to zero gas_limit
        assert!(result.is_err());
        assert_eq!(mp.size(), 0);
    }

    #[test]
    fn test_get_pending_ordered() {
        let mut mp = Mempool::new(10);
        // Add transactions with different gas prices
        mp.add_transaction(make_tx(1, 2, 0, 21_000, 10)).unwrap();
        mp.add_transaction(make_tx(1, 2, 1, 21_000, 50)).unwrap();
        mp.add_transaction(make_tx(1, 2, 2, 21_000, 30)).unwrap();

        let pending = mp.get_pending_transactions(10);
        assert_eq!(pending.len(), 3);
        // Highest gas_price first
        assert_eq!(pending[0].gas_price, 50);
        assert_eq!(pending[1].gas_price, 30);
        assert_eq!(pending[2].gas_price, 10);
    }

    #[test]
    fn test_remove_transaction() {
        let mut mp = Mempool::new(10);
        let tx = make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE);
        let hash = tx.hash();

        mp.add_transaction(tx).unwrap();
        assert_eq!(mp.size(), 1);

        let removed = mp.remove_transaction(&hash);
        assert!(removed.is_some());
        assert_eq!(mp.size(), 0);

        // Removing again returns None
        assert!(mp.remove_transaction(&hash).is_none());
    }

    #[test]
    fn test_size_and_clear() {
        let mut mp = Mempool::new(10);
        mp.add_transaction(make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE)).unwrap();
        mp.add_transaction(make_tx(1, 2, 1, 21_000, constants::BASE_GAS_PRICE)).unwrap();
        mp.add_transaction(make_tx(2, 3, 0, 21_000, constants::BASE_GAS_PRICE)).unwrap();

        assert_eq!(mp.size(), 3);
        assert!(!mp.is_empty());

        mp.clear();
        assert_eq!(mp.size(), 0);
        assert!(mp.is_empty());
    }

    #[test]
    fn test_contains() {
        let mut mp = Mempool::new(10);
        let tx = make_tx(1, 2, 0, 21_000, constants::BASE_GAS_PRICE);
        let hash = tx.hash();

        assert!(!mp.contains(&hash));
        mp.add_transaction(tx).unwrap();
        assert!(mp.contains(&hash));

        mp.remove_transaction(&hash);
        assert!(!mp.contains(&hash));
    }
}