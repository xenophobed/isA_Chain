use isa_chain_core::types::{Address, Amount};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AccountType {
    External,
    Contract,
    MultiSig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WalletAccount {
    pub address: Address,
    pub account_type: AccountType,
    pub derivation_path: String,
    pub name: String,
    pub balance: Amount,
    pub nonce: u64,
}

impl WalletAccount {
    pub fn new(
        address: Address,
        account_type: AccountType,
        derivation_path: String,
        name: String,
        balance: Amount,
    ) -> Self {
        WalletAccount {
            address,
            account_type,
            derivation_path,
            name,
            balance,
            nonce: 0,
        }
    }
}