use super::*;

#[test]
fn test_hash_creation() {
    let data = b"hello world";
    let hash = Hash::hash_data(data);

    // Should be deterministic
    let hash2 = Hash::hash_data(data);
    assert_eq!(hash, hash2);

    // Different data should produce different hash
    let hash3 = Hash::hash_data(b"hello world!");
    assert_ne!(hash, hash3);
}

#[test]
fn test_address_from_public_key() {
    let public_key = [1u8; 32];
    let address = Address::from_public_key(&public_key);

    // Should be deterministic
    let address2 = Address::from_public_key(&public_key);
    assert_eq!(address, address2);
}

#[test]
fn test_merkle_root() {
    let hashes = vec![
        Hash::hash_data(b"tx1"),
        Hash::hash_data(b"tx2"),
        Hash::hash_data(b"tx3"),
    ];

    let root = Hash::merkle_root(&hashes);
    assert_ne!(root, Hash::ZERO);

    // Empty list should return zero hash
    let empty_root = Hash::merkle_root(&[]);
    assert_eq!(empty_root, Hash::ZERO);
}

#[test]
fn test_signature_serialization() {
    let sig = Signature::new([1u8; 32], [2u8; 32], 27);
    let bytes = sig.to_bytes();
    let sig2 = Signature::from_bytes(&bytes).unwrap();

    assert_eq!(sig, sig2);
}
