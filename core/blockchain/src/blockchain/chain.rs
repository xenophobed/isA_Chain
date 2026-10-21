use crate::account::Account;
use crate::block::Block;
use crate::error::*;
use crate::mempool::Mempool;
use crate::staking::StakingVault;
use crate::storage::{RocksDbStorage, Storage};
use crate::token::{TokenError, TokenState, TokenSupplyInfo};
use crate::treasury::ProtocolTreasury;
use crate::types::*;
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
    pub(crate) blocks: HashMap<Hash, Block>,

    /// Account state (in-memory)
    pub(crate) accounts: HashMap<Address, Account>,

    /// Transaction mempool
    pub(crate) mempool: Mempool,

    /// Native ISA token state (supply accounting + authority lists)
    pub(crate) token_state: TokenState,

    /// Protocol treasury for fee collection
    pub(crate) treasury: ProtocolTreasury,

    /// Staking vault for validators/providers
    pub(crate) staking_vault: StakingVault,

    /// Optional persistent storage (None = in-memory only)
    pub(crate) storage: Option<RocksDbStorage>,
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

    /// Number of accounts currently loaded in memory.
    pub fn account_count(&self) -> usize {
        self.accounts.len()
    }
}
