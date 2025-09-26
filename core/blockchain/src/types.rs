use serde::{Deserialize, Serialize};
use std::fmt;

/// Chain ID type
pub type ChainId = u64;

/// Block height type
pub type BlockHeight = u64;

/// Gas amount type
pub type Gas = u64;

/// Gas price type
pub type GasPrice = u64;

/// Amount/Balance type using u128 for large numbers
pub type Amount = u128;

/// Timestamp type (Unix timestamp in milliseconds)
pub type Timestamp = u64;

/// Hash type - 32 bytes using Blake3
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hash([u8; 32]);

impl Hash {
    pub const ZERO: Self = Hash([0u8; 32]);
    
    pub fn new(data: [u8; 32]) -> Self {
        Hash(data)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 32 {
            return Err("Hash must be 32 bytes");
        }
        let mut array = [0u8; 32];
        array.copy_from_slice(bytes);
        Ok(Hash(array))
    }
    
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
    
    /// Generate hash from data using Blake3
    pub fn hash_data(data: &[u8]) -> Self {
        let hash = blake3::hash(data);
        Hash(*hash.as_bytes())
    }
    
    /// Generate Merkle root from list of hashes
    pub fn merkle_root(hashes: &[Hash]) -> Self {
        if hashes.is_empty() {
            return Hash::ZERO;
        }
        
        if hashes.len() == 1 {
            return hashes[0];
        }
        
        let mut current_level = hashes.to_vec();
        
        while current_level.len() > 1 {
            let mut next_level = Vec::new();
            
            for chunk in current_level.chunks(2) {
                let combined = if chunk.len() == 2 {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined.extend_from_slice(chunk[1].as_bytes());
                    combined
                } else {
                    let mut combined = Vec::new();
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined.extend_from_slice(chunk[0].as_bytes());
                    combined
                };
                next_level.push(Hash::hash_data(&combined));
            }
            
            current_level = next_level;
        }
        
        current_level[0]
    }
}

impl fmt::Debug for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Hash({})", hex::encode(&self.0[..8]))
    }
}

impl fmt::Display for Hash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

impl From<[u8; 32]> for Hash {
    fn from(bytes: [u8; 32]) -> Self {
        Hash(bytes)
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Address type - 20 bytes derived from public key
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Address([u8; 20]);

impl Address {
    pub const ZERO: Self = Address([0u8; 20]);
    
    pub fn new(data: [u8; 20]) -> Self {
        Address(data)
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 20 {
            return Err("Address must be 20 bytes");
        }
        let mut array = [0u8; 20];
        array.copy_from_slice(bytes);
        Ok(Address(array))
    }
    
    pub fn from_public_key(public_key: &[u8]) -> Self {
        let hash = blake3::hash(public_key);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash.as_bytes()[12..32]);
        Address(address)
    }
    
    pub fn as_bytes(&self) -> &[u8; 20] {
        &self.0
    }
    
    pub fn to_vec(&self) -> Vec<u8> {
        self.0.to_vec()
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Address({})", hex::encode(&self.0[..4]))
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{}", hex::encode(&self.0))
    }
}

impl From<[u8; 20]> for Address {
    fn from(bytes: [u8; 20]) -> Self {
        Address(bytes)
    }
}

impl AsRef<[u8]> for Address {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

/// Signature type
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Signature {
    pub r: [u8; 32],
    pub s: [u8; 32],
    pub v: u8,
}

impl Signature {
    pub fn new(r: [u8; 32], s: [u8; 32], v: u8) -> Self {
        Signature { r, s, v }
    }
    
    pub fn to_bytes(&self) -> [u8; 65] {
        let mut bytes = [0u8; 65];
        bytes[0..32].copy_from_slice(&self.r);
        bytes[32..64].copy_from_slice(&self.s);
        bytes[64] = self.v;
        bytes
    }
    
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &'static str> {
        if bytes.len() != 65 {
            return Err("Signature must be 65 bytes");
        }
        
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&bytes[0..32]);
        s.copy_from_slice(&bytes[32..64]);
        let v = bytes[64];
        
        Ok(Signature { r, s, v })
    }
}

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Signature(r: {}, s: {}, v: {})", 
               hex::encode(&self.r[..4]),
               hex::encode(&self.s[..4]),
               self.v)
    }
}

/// Network-specific constants
pub mod constants {
    use super::*;
    
    pub const MAIN_CHAIN_ID: ChainId = 15489;
    pub const TEST_CHAIN_ID: ChainId = 15490;
    
    pub const BLOCK_TIME_MS: u64 = 3000; // 3 seconds
    pub const MAX_BLOCK_SIZE: usize = 1024 * 1024; // 1MB
    pub const MAX_GAS_PER_BLOCK: Gas = 30_000_000;
    pub const BASE_GAS_PRICE: GasPrice = 1_000_000_000; // 1 Gwei
    
    pub const GENESIS_TIMESTAMP: Timestamp = 1704067200000; // Jan 1, 2024 00:00:00 UTC
    pub const INITIAL_SUPPLY: Amount = 1_000_000_000_000_000_000_000_000_000; // 1B ISA tokens
    
    pub const VALIDATOR_MIN_STAKE: Amount = 32_000_000_000_000_000_000_000; // 32,000 ISA
    pub const DELEGATION_MIN_AMOUNT: Amount = 1_000_000_000_000_000_000; // 1 ISA
}

#[cfg(test)]
mod tests {
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
}