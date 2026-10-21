use isa_chain_core::{
    constants::{BASE_GAS_PRICE, MAIN_CHAIN_ID},
    transaction::{Transaction, TransactionData},
    Address,
};

/// Construct a deterministic `Address` from a u16 index without collision.
pub fn addr(i: u16) -> Address {
    let lo = (i & 0xFF) as u8;
    let hi = ((i >> 8) & 0xFF) as u8;
    let mut bytes = [0u8; 20];
    bytes[0] = hi;
    bytes[1] = lo;
    Address::from(bytes)
}

/// Build an unsigned Transfer transaction.  `gas_limit` = 21_000 matches the
/// protocol base cost; `fee() = gas_limit * gas_price` is deducted from the
/// sender's balance during `submit_transaction`.
pub fn make_transfer(from: Address, to: Address, nonce: u64, amount: u128) -> Transaction {
    Transaction::new(
        from,
        nonce,
        TransactionData::Transfer {
            to,
            amount,
            data: vec![],
        },
        21_000,
        BASE_GAS_PRICE,
        MAIN_CHAIN_ID,
    )
}
