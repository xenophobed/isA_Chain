// Placeholder for transaction mempool
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
        
        // TODO: Validate transaction
        // TODO: Check nonce ordering
        // TODO: Check gas price
        
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