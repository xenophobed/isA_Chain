// Placeholder for main blockchain logic
use crate::types::*;
use crate::block::Block;
use crate::transaction::Transaction;
use crate::account::Account;
use crate::token::{TokenState, TokenSupplyInfo, TokenError};
use crate::error::*;
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

    /// Native ISA token state (supply accounting + authority lists)
    token_state: TokenState,
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
            token_state: TokenState::new(constants::INITIAL_SUPPLY, Address::ZERO),
        }
    }

    pub fn get_block(&self, hash: &Hash) -> Option<&Block> {
        self.blocks.get(hash)
    }

    pub fn get_account(&self, address: &Address) -> Option<&Account> {
        self.accounts.get(address)
    }

    pub fn get_balance(&self, address: &Address) -> Amount {
        self.accounts
            .get(address)
            .map(|account| account.balance)
            .unwrap_or(0)
    }

    /// Initialize accounts with balance (for testing / genesis setup).
    ///
    /// This is the original mint helper — it credits the account without
    /// going through token authority checks so that genesis allocation
    /// and test setups keep working.
    pub fn mint(&mut self, address: Address, amount: Amount) {
        self.accounts
            .entry(address)
            .and_modify(|acc| acc.balance += amount)
            .or_insert_with(|| Account::new_external(amount));
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

    /// Mint new ISA tokens to `to`, going through token authority.
    ///
    /// The `minter` address must have been previously authorized via
    /// `token_state_mut().authorize_minter(...)`.
    pub fn mint_tokens(
        &mut self,
        to: Address,
        amount: Amount,
        minter: &Address,
    ) -> Result<(), BlockchainError> {
        // 1. Supply accounting (checks authorization + overflow)
        self.token_state.mint(amount, minter)?;

        // 2. Credit the recipient account
        self.accounts
            .entry(to)
            .and_modify(|acc| acc.balance += amount)
            .or_insert_with(|| Account::new_external(amount));

        Ok(())
    }

    /// Burn ISA tokens from `from`, going through token authority.
    ///
    /// The `burner` address must have been previously authorized via
    /// `token_state_mut().authorize_burner(...)`.
    pub fn burn_tokens(
        &mut self,
        from: Address,
        amount: Amount,
        burner: &Address,
    ) -> Result<(), BlockchainError> {
        // 1. Verify the account has enough balance
        let balance = self.get_balance(&from);
        if balance < amount {
            return Err(TokenError::InsufficientBalance.into());
        }

        // 2. Supply accounting (checks authorization)
        self.token_state.burn(amount, burner)?;

        // 3. Deduct from account
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

    // TODO: Implement full blockchain logic
    // - Block validation and addition
    // - Transaction execution
    // - State transitions
    // - Fork resolution
    // - Finality tracking
}