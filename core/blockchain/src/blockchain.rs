use crate::types::*;
use crate::block::Block;
use crate::transaction::Transaction;
use crate::account::Account;
use crate::token::{TokenState, TokenSupplyInfo, TokenError};
use crate::mempool::Mempool;
use crate::error::*;
use crate::treasury::ProtocolTreasury;
use crate::staking::StakingVault;
use crate::storage::{RocksDbStorage, Storage};
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

    /// Block storage (in-memory)
    blocks: HashMap<Hash, Block>,

    /// Account state (in-memory)
    accounts: HashMap<Address, Account>,

    /// Transaction mempool
    mempool: Mempool,

    /// Native ISA token state (supply accounting + authority lists)
    token_state: TokenState,

    /// Protocol treasury for fee collection
    treasury: ProtocolTreasury,

    /// Staking vault for validators/providers
    staking_vault: StakingVault,

    /// Optional persistent storage (None = in-memory only)
    storage: Option<RocksDbStorage>,
}

impl Blockchain {
    /// Create a new in-memory blockchain (no persistence).
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
            storage: None,
        }
    }

    /// Create a blockchain backed by RocksDB persistent storage.
    ///
    /// If the database already contains state (non-zero latest height), the
    /// in-memory caches are restored from storage so the chain can continue
    /// from where it left off.  Otherwise a genesis block is created and
    /// immediately persisted.
    pub fn new_with_storage(chain_id: ChainId, mut storage: RocksDbStorage) -> Result<Self, BlockchainError> {
        let latest_height = storage.get_latest_height()
            .map_err(BlockchainError::Storage)?;

        if let Some(height) = latest_height {
            // ── Restore existing chain ──────────────────────────────────────
            // Walk all blocks from genesis to head and load them into memory.
            let mut blocks: HashMap<Hash, Block> = HashMap::new();
            let mut head = Hash::ZERO;
            let mut genesis_hash = Hash::ZERO;

            for h in 0..=height {
                let block = storage
                    .get_block_by_height(h)
                    .map_err(BlockchainError::Storage)?
                    .ok_or(BlockchainError::Storage(StorageError::DataCorruption { height: h }))?;

                let hash = block.hash();
                if h == 0 { genesis_hash = hash; }
                if h == height { head = hash; }
                blocks.insert(hash, block);
            }

            Ok(Blockchain {
                chain_id,
                head,
                height,
                genesis_hash,
                blocks,
                accounts: HashMap::new(), // accounts are lazily loaded; see get_account_or_storage
                mempool: Mempool::new(10000),
                token_state: TokenState::new(constants::INITIAL_SUPPLY, Address::ZERO),
                treasury: ProtocolTreasury::new(constants::PROTOCOL_FEE_PERCENT, Address::ZERO),
                staking_vault: StakingVault::new(constants::VALIDATOR_MIN_STAKE, 100),
                storage: Some(storage),
            })
        } else {
            // ── Brand-new chain: create and persist genesis ─────────────────
            let genesis = Block::genesis(chain_id);
            let genesis_hash = genesis.hash();

            storage.put_block(genesis.clone())
                .map_err(BlockchainError::Storage)?;

            let mut blocks = HashMap::new();
            blocks.insert(genesis_hash, genesis);

            Ok(Blockchain {
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
                storage: Some(storage),
            })
        }
    }

    /// Whether this blockchain instance has persistent storage enabled.
    pub fn has_storage(&self) -> bool {
        self.storage.is_some()
    }


    pub fn get_block(&self, hash: &Hash) -> Option<&Block> {
        self.blocks.get(hash)
    }

    pub fn get_account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    /// Look up an account, checking persistent storage on a cache miss.
    pub fn get_account_mut_or_load(&mut self, address: &Address) -> Option<Account> {
        if let Some(acc) = self.accounts.get(address) {
            return Some(acc.clone());
        }
        if let Some(ref mut db) = self.storage {
            if let Ok(Some(acc)) = db.get_account(address) {
                self.accounts.insert(*address, acc.clone());
                return Some(acc);
            }
        }
        None
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
                        acc.balance += amount;
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

    /// Initialize accounts with balance (for testing / genesis setup).
    /// This bypasses token authority checks.
    pub fn mint(&mut self, address: Address, amount: Amount) {
        self.accounts.entry(address)
            .and_modify(|acc| acc.balance += amount)
            .or_insert_with(|| Account::new_external(amount));

        if let Some(ref mut db) = self.storage {
            if let Some(acc) = self.accounts.get(&address) {
                let _ = db.put_account(address, acc.clone());
            }
        }
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
            .ok_or(ValidationError::InsufficientBalance)?
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

    /// Number of accounts currently loaded in memory.
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::{Block, ConsensusData, StakeData};

    fn make_blockchain() -> Blockchain {
        Blockchain::new(constants::MAIN_CHAIN_ID)
    }

    fn make_empty_block(height: BlockHeight, parent_hash: Hash) -> Block {
        Block::new(
            height,
            parent_hash,
            vec![],
            Hash::hash_data(b"state_root"),
            Hash::hash_data(b"receipts_root"),
            Address::ZERO,
            constants::MAX_GAS_PER_BLOCK,
            ConsensusData {
                validator_signatures: vec![],
                stake_data: StakeData {
                    total_stake: 0,
                    min_stake: constants::VALIDATOR_MIN_STAKE,
                    slash_penalties: vec![],
                },
                randomness: Hash::hash_data(b"randomness"),
            },
        )
    }

    #[test]
    fn test_new_blockchain() {
        let bc = make_blockchain();
        assert_eq!(bc.height, 0);
        assert_ne!(bc.genesis_hash, Hash::ZERO);
        assert_eq!(bc.head, bc.genesis_hash);
    }

    #[test]
    fn test_get_balance_empty() {
        let bc = make_blockchain();
        let unknown = Address::from([0xABu8; 20]);
        assert_eq!(bc.get_balance(&unknown), 0);
    }

    #[test]
    fn test_mint_credits_balance() {
        let mut bc = make_blockchain();
        let addr = Address::from([1u8; 20]);
        bc.mint(addr, 5_000);
        assert_eq!(bc.get_balance(&addr), 5_000);
    }

    #[test]
    fn test_add_block() {
        let mut bc = make_blockchain();
        let parent_hash = bc.head;
        let block = make_empty_block(1, parent_hash);
        assert!(bc.add_block(block).is_ok());
        assert_eq!(bc.height, 1);
    }

    #[test]
    fn test_add_block_wrong_height() {
        let mut bc = make_blockchain();
        let parent_hash = bc.head;
        // height 2 is wrong — chain is at 0, expects 1
        let block = make_empty_block(2, parent_hash);
        let result = bc.add_block(block);
        assert!(result.is_err());
    }

    #[test]
    fn test_add_block_wrong_parent() {
        let mut bc = make_blockchain();
        // use a wrong parent hash
        let wrong_parent = Hash::hash_data(b"not_the_real_parent");
        let block = make_empty_block(1, wrong_parent);
        let result = bc.add_block(block);
        assert!(result.is_err());
    }

    #[test]
    fn test_token_state_accessible() {
        let bc = make_blockchain();
        let supply = bc.token_state().get_total_supply();
        assert_eq!(supply, constants::INITIAL_SUPPLY);
    }

    #[test]
    fn test_treasury_accessible() {
        let bc = make_blockchain();
        // Verify the accessor compiles and returns the struct without panic
        let _treasury = bc.treasury();
    }

    #[test]
    fn test_staking_vault_accessible() {
        let bc = make_blockchain();
        let _vault = bc.staking_vault();
    }

    #[test]
    fn test_produce_block() {
        let mut bc = make_blockchain();
        let result = bc.produce_block(100);
        assert!(result.is_ok());
        assert_eq!(bc.height, 1);
    }

    /// Settlement transaction: gross_amount deducted from user, net credited to
    /// provider, fee credited to treasury (Address::ZERO in tests).
    #[test]
    fn test_execute_settlement_transaction() {
        use crate::transaction::{Transaction, TransactionData};

        let user_addr    = Address::from([0x01u8; 20]);
        let provider_addr = Address::from([0x02u8; 20]);
        let gross: Amount = 10_000;
        let fee: Amount   = 250;
        let net: Amount   = gross - fee; // 9_750

        let mut bc = make_blockchain();
        bc.mint(user_addr, gross);
        assert_eq!(bc.get_balance(&user_addr), gross);

        // Seed a nonce-0 account so the entry exists in the map.
        bc.accounts.entry(user_addr)
            .and_modify(|acc| acc.nonce = 0);

        let tx = Transaction::new(
            user_addr,
            0,
            TransactionData::Settlement {
                user: user_addr,
                provider: provider_addr,
                gross_amount: gross,
                fee_amount: fee,
            },
            21_000,
            constants::BASE_GAS_PRICE,
            constants::MAIN_CHAIN_ID,
        );

        assert!(bc.execute_transaction(&tx).is_ok());

        // User balance fully drained.
        assert_eq!(bc.get_balance(&user_addr), 0);
        // Provider receives net amount.
        assert_eq!(bc.get_balance(&provider_addr), net);
        // Fee lands at treasury address (Address::ZERO).
        assert_eq!(bc.get_balance(&Address::ZERO), fee);
    }
}
