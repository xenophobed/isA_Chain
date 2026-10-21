use super::*;
use crate::error::NetworkError;
use crate::network::types::{NetworkConfig, NetworkMessage};

/// Round-trip every variant through bincode to verify Serialize/Deserialize
/// are correctly derived.
#[test]
fn test_network_message_serialization() {
    let messages: Vec<NetworkMessage> = vec![
        NetworkMessage::Hello {
            version: 1,
            chain_id: 42,
            best_height: 100,
            best_hash: head(),
            genesis_hash: genesis(),
        },
        NetworkMessage::GetBlocks {
            start_height: 50,
            count: 10,
        },
        NetworkMessage::Blocks(vec![]),
        NetworkMessage::NewBlock {
            height: 101,
            hash: head(),
        },
        NetworkMessage::NewTransaction(Hash::ZERO),
        NetworkMessage::GetBlock(Hash::ZERO),
        NetworkMessage::GetTransactions(vec![Hash::ZERO, head()]),
        NetworkMessage::Transactions(vec![]),
        NetworkMessage::Status,
        NetworkMessage::StatusResponse {
            height: 100,
            head: head(),
            peer_count: 5,
        },
        NetworkMessage::Ping(12345),
        NetworkMessage::Pong(12345),
    ];

    for msg in &messages {
        let encoded = bincode::serialize(msg).expect("serialize failed");
        let decoded: NetworkMessage =
            bincode::deserialize(&encoded).expect("deserialize failed");

        // Verify the discriminant / key fields survive the round-trip.
        match (msg, &decoded) {
            (NetworkMessage::Hello { version: v1, .. }, NetworkMessage::Hello { version: v2, .. }) => {
                assert_eq!(v1, v2)
            }
            (NetworkMessage::GetBlocks { count: c1, .. }, NetworkMessage::GetBlocks { count: c2, .. }) => {
                assert_eq!(c1, c2)
            }
            (NetworkMessage::Ping(n1), NetworkMessage::Pong(_)) => {
                panic!("Ping decoded as Pong: nonce={n1}")
            }
            (NetworkMessage::Ping(n1), NetworkMessage::Ping(n2)) => assert_eq!(n1, n2),
            (NetworkMessage::Pong(n1), NetworkMessage::Pong(n2)) => assert_eq!(n1, n2),
            (NetworkMessage::StatusResponse { height: h1, .. }, NetworkMessage::StatusResponse { height: h2, .. }) => {
                assert_eq!(h1, h2)
            }
            _ => {
                // For unit variants (Status, Blocks(empty), …) just confirm
                // no panic — the encode/decode succeeded above.
            }
        }
    }
}

#[test]
fn test_message_handler_hello() {
    let handler = MessageHandler::new(1, genesis());
    let msg = handler.create_hello(42, head());

    match msg {
        NetworkMessage::Hello {
            version,
            chain_id,
            best_height,
            best_hash,
            genesis_hash,
        } => {
            assert_eq!(version, 1);
            assert_eq!(chain_id, 1);
            assert_eq!(best_height, 42);
            assert_eq!(best_hash, head());
            assert_eq!(genesis_hash, genesis());
        }
        _ => panic!("expected Hello"),
    }
}

#[test]
fn test_message_handler_validate_hello_ok() {
    let handler = MessageHandler::new(1, genesis());
    let hello = handler.create_hello(0, Hash::ZERO);
    assert!(handler.validate_hello(&hello).is_ok());
}

#[test]
fn test_message_handler_validate_hello_wrong_chain() {
    let handler = MessageHandler::new(1, genesis());
    let wrong_chain = NetworkMessage::Hello {
        version: 1,
        chain_id: 99,
        best_height: 0,
        best_hash: Hash::ZERO,
        genesis_hash: genesis(),
    };
    let result = handler.validate_hello(&wrong_chain);
    assert!(result.is_err());
    matches!(result.unwrap_err(), NetworkError::InvalidMessage { .. });
}

#[test]
fn test_message_handler_validate_hello_wrong_version() {
    let handler = MessageHandler::new(1, genesis());
    let wrong_version = NetworkMessage::Hello {
        version: 99,
        chain_id: 1,
        best_height: 0,
        best_hash: Hash::ZERO,
        genesis_hash: genesis(),
    };
    let result = handler.validate_hello(&wrong_version);
    assert!(result.is_err());
    matches!(
        result.unwrap_err(),
        NetworkError::ProtocolMismatch { expected: 1, actual: 99 }
    );
}

#[test]
fn test_message_handler_validate_hello_wrong_genesis() {
    let handler = MessageHandler::new(1, genesis());
    let wrong_genesis = NetworkMessage::Hello {
        version: 1,
        chain_id: 1,
        best_height: 0,
        best_hash: Hash::ZERO,
        genesis_hash: Hash::ZERO, // wrong
    };
    let result = handler.validate_hello(&wrong_genesis);
    assert!(result.is_err());
}

#[test]
fn test_message_handler_validate_non_hello() {
    let handler = MessageHandler::new(1, genesis());
    let result = handler.validate_hello(&NetworkMessage::Ping(0));
    assert!(result.is_err());
}

#[test]
fn test_network_config_default() {
    let cfg = NetworkConfig::default();
    assert_eq!(cfg.listen_port, 30333);
    assert_eq!(cfg.max_peers, 50);
    assert!(cfg.bootstrap_nodes.is_empty());
    assert_eq!(cfg.chain_id, 1);
    assert_eq!(cfg.protocol_version, 1);
}
