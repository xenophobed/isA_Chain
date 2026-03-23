use crate::types::*;
use crate::block::Block;
use crate::transaction::Transaction;
use crate::account::Account;
use crate::token::{TokenState, TokenSupplyInfo, TokenError};
use crate::mempool::Mempool;
use crate::error::*;
use crate::treasury::ProtocolTreasury;
use crate::staking::StakingVault;
use std::collections::HashMap;

/// Main blockchain state and logic
pub struct Blockchain {
    /// Chain ID
    pub chain_id: ChainId,

    /// Current chain head
    pub head: Hash,

    /// Current block height
    pub height: BlockHeight,

    /// Genesis block hash
    pub genesis_hash: Hash,

    /// Block storage
    blocks: HashMap<Hash, Block>,

    /// Account state
    accounts: HashMap<Address, Account>,

    /// Transaction mempool
    mempool: Mempool,

    /// Native ISA token state (supply accounting + authority lists)
    token_state: TokenState,

    /// Protocol treasury for fee collection
    treasury: ProtocolTreasury,
    /// Staking vault for validators/providers
    staking_vault: StakingVault,
}

impl Blockchain {
    pub fn new(chain_id: ChainId) -> Self {
        let genesis = Block::genesis(chain_id);
        let genesis_hash = genesis.hash();

        let mut blocks = HashMap::new();
        blocks.insert(genesis_hash, genesis);

        Blockchain {
            chain_id,
            head: genesis_hash,
            height: 0,
            genesis_hash,
            blocks,
            accounts: HashMap::new(),
            mempool: Mempool::new(10000),
            token_state: TokenState::new(constants::INITIAL_SUPPLY, Address::ZERO),
            treasury: ProtocolTreasury::new(constants::PROTOCOL_FEE_PERCENT, Address::ZERO),
            staking_vault: StakingVault::new(constants::VALIDATOR_MIN_STAKE, 100),
        }
    }

    pub fn get_block(&self, hash: &Hash) -> Option<&Block> {
        self.blocks.get(hash)
    }

    pub fn get_account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    pub fn get_height(&self) -> BlockHeight {
        self.height
    }

    pub fn get_balance(&self, address: &Address) -> Amount {
        self.accounts
            .get(address)
            .map(|account| account.balance)
            .unwrap_or(0)
    }

    pub fn get_nonce(&self, address: &Address) -> u64 {
        self.accounts
            .get(address)
            .map(|account| account.nonce)
            .unwrap_or(0)
    }

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

        self.blocks.insert(block_hash, block);
        self.head = block_hash;
        self.height += 1;

        Ok(())
    }

    fn execute_transaction(&mut self, tx: &Transaction) -> Result<(), BlockchainError> {
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
            _ => {
                // TODO: Implement other transaction types
            }
        }

        Ok(())
    }

    /// Initialize accounts with balance (for testing / genesis setup).
    /// This bypasses token authority checks.
    pub fn mint(&mut self, address: Address, amount: Amount) {
        self.accounts.entry(address)
            .and_modify(|acc| acc.balance += amount)
            .or_insert_with(|| Account::new_external(amount));
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

    // ----------------------------------------------------------------
    // Token-state integration
    // ----------------------------------------------------------------

    /// Access the underlying token state (read-only).
    pub fn token_state(&self) -> &TokenState {
        &self.token_state
    }

    /// Access the underlying token state (mutable).
    pub fn token_state_mut(&mut self) -> &mut TokenState {
        &mut self.token_state
    }

    pub fn treasury(&self) -> &ProtocolTreasury { &self.treasury }
    pub fn treasury_mut(&mut self) -> &mut ProtocolTreasury { &mut self.treasury }
    pub fn staking_vault(&self) -> &StakingVault { &self.staking_vault }
    pub fn staking_vault_mut(&mut self) -> &mut StakingVault { &mut self.staking_vault }

    /// Mint new ISA tokens to `to`, going through token authority.
    pub fn mint_tokens(
        &mut self,
        to: Address,
        amount: Amount,
        minter: &Address,
    ) -> Result<(), BlockchainError> {
        self.token_state.mint(amount, minter)?;

        self.accounts
            .entry(to)
            .and_modify(|acc| acc.balance += amount)
            .or_insert_with(|| Account::new_external(amount));

        Ok(())
    }

    /// Burn ISA tokens from `from`, going through token authority.
    pub fn burn_tokens(
        &mut self,
        from: Address,
        amount: Amount,
        burner: &Address,
    ) -> Result<(), BlockchainError> {
        let balance = self.get_balance(&from);
        if balance < amount {
            return Err(TokenError::InsufficientBalance.into());
        }

        self.token_state.burn(amount, burner)?;

        self.accounts
            .get_mut(&from)
            .expect("account must exist after balance check")
            .balance -= amount;

        Ok(())
    }

    /// Return a snapshot of the ISA token supply.
    pub fn get_token_supply(&self) -> TokenSupplyInfo {
        self.token_state.get_supply_info()
    }

    /// Get a block by height by scanning block storage.
    pub fn get_block_by_height(&self, height: BlockHeight) -> Option<&Block> {
        if height == 0 {
            return self.blocks.get(&self.genesis_hash);
        }
        self.blocks.values().find(|b| b.header.height == height)
    }

    /// Get the latest (head) block.
    pub fn get_latest_block(&self) -> Option<&Block> {
        self.blocks.get(&self.head)
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
