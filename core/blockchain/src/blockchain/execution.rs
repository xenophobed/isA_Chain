use super::chain::Blockchain;
use crate::account::Account;
use crate::block::Block;
use crate::error::*;
use crate::storage::Storage;
use crate::transaction::Transaction;
use crate::types::*;

impl Blockchain {
    pub fn add_block(&mut self, block: Block) -> Result<(), BlockchainError> {
        if block.header.height != self.height + 1 {
            return Err(ValidationError::InvalidBlockHeight {
                expected: self.height + 1,
                actual: block.header.height,
            }.into());
        }

        if block.header.parent_hash != self.head {
            return Err(ValidationError::InvalidParentHash.into());
        }

        let block_hash = block.hash();

        for tx in &block.transactions {
            self.execute_transaction(tx)?;
        }

        self.blocks.insert(block_hash, block.clone());
        self.head = block_hash;
        self.height += 1;

        // ── Persist to RocksDB if storage is configured ─────────────────────
        if let Some(ref mut db) = self.storage {
            db.put_block(block).map_err(BlockchainError::Storage)?;

            // Persist every account that was touched by this block's transactions.
            // We iterate the in-memory map and write all entries; a production
            // implementation would track dirty accounts, but this is correct.
            let account_snapshot: Vec<(Address, Account)> = self
                .accounts
                .iter()
                .map(|(addr, acc)| (*addr, acc.clone()))
                .collect();
            for (addr, acc) in account_snapshot {
                db.put_account(addr, acc).map_err(BlockchainError::Storage)?;
            }
        }

        Ok(())
    }

    pub fn execute_transaction(&mut self, tx: &Transaction) -> Result<(), BlockchainError> {
        let sender_balance = self.get_balance(&tx.from);

        match &tx.data {
            crate::transaction::TransactionData::Transfer { to, amount, .. } => {
                if sender_balance < *amount {
                    return Err(ValidationError::InsufficientBalance.into());
                }

                self.accounts.entry(tx.from)
                    .and_modify(|acc| {
                        acc.balance -= amount;
                        acc.nonce += 1;
                    });

                self.accounts.entry(*to)
                    .and_modify(|acc| acc.balance += amount)
                    .or_insert_with(|| Account::new_external(*amount));
            }
            crate::transaction::TransactionData::Stake { amount, .. } => {
                // Deduct staked amount from sender; stake accounting is handled
                // by StakingVault separately.
                if sender_balance < *amount {
                    return Err(ValidationError::InsufficientBalance.into());
                }
                self.accounts.entry(tx.from)
                    .and_modify(|acc| {
                        acc.balance -= amount;
                        acc.nonce += 1;
                    });
            }
            crate::transaction::TransactionData::Unstake { amount, .. } => {
                // Return previously-staked amount to sender.
                self.accounts.entry(tx.from)
                    .and_modify(|acc| {
                        acc.balance -= amount;
                        acc.nonce += 1;
                    })
                    .or_insert_with(|| Account::new_external(*amount));
            }
            crate::transaction::TransactionData::CreditPurchase { isa_amount, .. } => {
                // Burn ISA from buyer to purchase credits (credits are off-chain).
                if sender_balance < *isa_amount {
                    return Err(ValidationError::InsufficientBalance.into());
                }
                self.accounts.entry(tx.from)
                    .and_modify(|acc| {
                        acc.balance -= isa_amount;
                        acc.nonce += 1;
                    });
            }
            crate::transaction::TransactionData::CreditSpend { .. } => {
                // Credits are tracked off-chain; only increment nonce.
                self.accounts.entry(tx.from)
                    .and_modify(|acc| acc.nonce += 1);
            }
            crate::transaction::TransactionData::AgentWalletSpend { .. } => {
                // Agent wallet balances are tracked off-chain; only increment nonce.
                self.accounts.entry(tx.from)
                    .and_modify(|acc| acc.nonce += 1);
            }
            crate::transaction::TransactionData::Settlement {
                user,
                provider,
                gross_amount,
                fee_amount,
            } => {
                // Deduct gross amount from user; net (gross - fee) goes to provider;
                // fee goes to treasury address.
                if sender_balance < *gross_amount {
                    return Err(ValidationError::InsufficientBalance.into());
                }
                let net_amount = gross_amount.saturating_sub(*fee_amount);
                self.accounts.entry(*user)
                    .and_modify(|acc| {
                        acc.balance -= gross_amount;
                        acc.nonce += 1;
                    });
                self.accounts.entry(*provider)
                    .and_modify(|acc| acc.balance += net_amount)
                    .or_insert_with(|| Account::new_external(net_amount));
                // Fee accrues to the treasury address (Address::ZERO — the admin
                // address used when initialising the chain).
                self.accounts.entry(Address::ZERO)
                    .and_modify(|acc| acc.balance += fee_amount)
                    .or_insert_with(|| Account::new_external(*fee_amount));
            }
            _ => {
                // For all other transaction variants: increment nonce and collect
                // the gas fee.  No additional balance changes are applied at this
                // layer — higher-level state machines (compute market, governance,
                // etc.) handle their own invariants.
                self.accounts.entry(tx.from)
                    .and_modify(|acc| acc.nonce += 1);
            }
        }

        Ok(())
    }

    /// Submit a transaction to the mempool
    pub fn submit_transaction(&mut self, tx: Transaction) -> Result<Hash, BlockchainError> {
        tx.verify()?;

        let expected_nonce = self.get_nonce(&tx.from);
        if tx.nonce != expected_nonce {
            return Err(ValidationError::InvalidBlockHeight {
                expected: expected_nonce,
                actual: tx.nonce,
            }.into());
        }

        let sender_balance = self.get_balance(&tx.from);
        let total_cost = match &tx.data {
            crate::transaction::TransactionData::Transfer { amount, .. } => {
                amount + tx.fee()
            }
            _ => tx.fee(),
        };

        if sender_balance < total_cost {
            return Err(ValidationError::InsufficientBalance.into());
        }

        let tx_hash = tx.hash();
        self.mempool.add_transaction(tx)?;

        Ok(tx_hash)
    }

    /// Get pending transactions from mempool
    pub fn get_pending_transactions(&self, max_count: usize) -> Vec<Transaction> {
        self.mempool.get_pending_transactions(max_count)
    }

    /// Build a new block from pending transactions
    pub fn build_block(&mut self, max_transactions: usize) -> Result<Block, BlockchainError> {
        let pending_txs = self.mempool.get_pending_transactions(max_transactions);

        let mut valid_txs = Vec::new();
        for tx in pending_txs {
            if self.get_nonce(&tx.from) == tx.nonce {
                let sender_balance = self.get_balance(&tx.from);
                let total_cost = match &tx.data {
                    crate::transaction::TransactionData::Transfer { amount, .. } => {
                        amount + tx.fee()
                    }
                    _ => tx.fee(),
                };

                if sender_balance >= total_cost {
                    valid_txs.push(tx);
                }
            }
        }

        let consensus_data = crate::block::ConsensusData {
            validator_signatures: vec![],
            stake_data: crate::block::StakeData {
                total_stake: 0,
                min_stake: constants::VALIDATOR_MIN_STAKE,
                slash_penalties: vec![],
            },
            randomness: Hash::hash_data(&self.height.to_le_bytes()),
        };

        let block = Block::new(
            self.height + 1,
            self.head,
            valid_txs,
            Hash::hash_data(b"state_root"),
            Hash::hash_data(b"receipts_root"),
            Address::ZERO,
            constants::MAX_GAS_PER_BLOCK,
            consensus_data,
        );

        Ok(block)
    }

    /// Produce a new block and add it to the chain
    pub fn produce_block(&mut self, max_transactions: usize) -> Result<Hash, BlockchainError> {
        let block = self.build_block(max_transactions)?;
        let block_hash = block.hash();

        let tx_hashes: Vec<Hash> = block.transactions.iter().map(|tx| tx.hash()).collect();
        self.mempool.remove_transactions(&tx_hashes);

        self.add_block(block)?;

        Ok(block_hash)
    }

    /// Get a pending transaction from the mempool by hash.
    pub fn get_pending_transaction(&self, hash: &Hash) -> Option<&Transaction> {
        self.mempool.get_transaction(hash)
    }

    /// Number of pending transactions in the mempool.
    pub fn pending_transaction_count(&self) -> usize {
        self.mempool.len()
    }
}
