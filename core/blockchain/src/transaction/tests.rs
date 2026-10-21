use super::*;
use crate::types::*;

#[test]
fn test_transaction_creation_and_signing() {
    let private_key = [1u8; 32];

    // Derive the correct address from the private key
    let secp = secp256k1::Secp256k1::new();
    let secret_key = secp256k1::SecretKey::from_slice(&private_key).unwrap();
    let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
    let from = Address::from_public_key(&public_key.serialize());

    let mut tx = Transaction::new(
        from,
        0,
        TransactionData::Transfer {
            to: Address::from([2u8; 20]),
            amount: 1000,
            data: vec![],
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );

    // Sign transaction
    assert!(tx.sign(&private_key).is_ok());
    assert!(tx.is_signed());

    // Verify transaction
    assert!(tx.verify().is_ok());
}

#[test]
fn test_transaction_hash_consistency() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::Transfer {
            to: Address::from([2u8; 20]),
            amount: 1000,
            data: vec![],
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    
    let hash1 = tx.hash();
    let hash2 = tx.hash();
    
    assert_eq!(hash1, hash2);
}

#[test]
fn test_credit_purchase_tx() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::CreditPurchase {
            buyer: Address::from([1u8; 20]),
            isa_amount: 1_000_000,
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    assert_eq!(tx.tx_type(), TransactionType::CreditPurchase);
}

#[test]
fn test_credit_spend_tx() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::CreditSpend {
            spender: Address::from([1u8; 20]),
            amount: 500,
            service: "inference".to_string(),
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    assert_eq!(tx.tx_type(), TransactionType::CreditSpend);
}

#[test]
fn test_agent_wallet_spend_tx() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::AgentWalletSpend {
            wallet: Address::from([2u8; 20]),
            amount: 100,
            service: "storage".to_string(),
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    assert_eq!(tx.tx_type(), TransactionType::AgentWalletSpend);
}

#[test]
fn test_settlement_tx() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::Settlement {
            user: Address::from([1u8; 20]),
            provider: Address::from([2u8; 20]),
            gross_amount: 10_000,
            fee_amount: 250,
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    assert_eq!(tx.tx_type(), TransactionType::Settlement);
}

#[test]
fn test_invalid_stake_amount() {
    let tx = Transaction::new(
        Address::from([1u8; 20]),
        0,
        TransactionData::Stake {
            amount: 1000, // Too low
            validator_info: ValidatorInfo {
                public_key: vec![1u8; 33],
                commission_rate: 1000,
                min_self_delegation: 1000,
                description: ValidatorDescription {
                    moniker: "test".to_string(),
                    identity: "".to_string(),
                    website: "".to_string(),
                    security_contact: "".to_string(),
                    details: "".to_string(),
                },
            },
        },
        21000,
        constants::BASE_GAS_PRICE,
        constants::MAIN_CHAIN_ID,
    );
    
    assert!(tx.validate_data().is_err());
}
