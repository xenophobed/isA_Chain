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
        write!(f, "0x{}", hex::encode(self.0))
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
        write!(f, "0x{}", hex::encode(self.0))
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
        write!(
            f,
            "Signature(r: {}, s: {}, v: {})",
            hex::encode(&self.r[..4]),
            hex::encode(&self.s[..4]),
            self.v
        )
    }
}
