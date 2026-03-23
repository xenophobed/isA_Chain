use thiserror::Error;
use isa_chain_core::types::Address;

/// Errors that can occur during address derivation
#[derive(Debug, Error)]
pub enum AddressDerivationError {
    #[error("Invalid seed: {0}")]
    InvalidSeed(String),

    #[error("Key derivation failed: {0}")]
    KeyDerivationFailed(String),

    #[error("Invalid public key: {0}")]
    InvalidPublicKey(String),
}

/// A derived ISA chain address with associated metadata
#[derive(Clone, Debug)]
pub struct DerivedAddress {
    /// Raw 20-byte ISA chain address
    pub address: [u8; 20],

    /// Hex-encoded address with 0x prefix
    pub address_hex: String,

    /// The public key bytes used to derive this address
    pub public_key: Vec<u8>,

    /// BIP44 derivation path used (or "direct" for public-key-only derivation)
    pub derivation_path: String,
}

/// Derives ISA chain addresses deterministically from seeds or public keys.
///
/// The address format is the last 20 bytes of the blake3 hash of the public key,
/// matching `isa_chain_core::types::Address::from_public_key`.
#[derive(Debug, Clone)]
pub struct AddressDeriver {
    /// Chain ID (BIP44 coin type)
    pub chain_id: u64,
}

impl AddressDeriver {
    /// Create a new `AddressDeriver` for the given chain.
    pub fn new(chain_id: u64) -> Self {
        AddressDeriver { chain_id }
    }

    /// Derive an ISA chain address from raw seed bytes.
    ///
    /// The seed is hashed with blake3 to produce a 32-byte synthetic "public key",
    /// then the address is derived from that key using the same algorithm as the chain.
    /// A deterministic BIP44-style path label is recorded for traceability.
    pub fn derive_from_seed(&self, seed: &[u8]) -> Result<DerivedAddress, AddressDerivationError> {
        if seed.is_empty() {
            return Err(AddressDerivationError::InvalidSeed(
                "seed must not be empty".to_string(),
            ));
        }
        if seed.len() < 16 {
            return Err(AddressDerivationError::InvalidSeed(format!(
                "seed must be at least 16 bytes, got {}",
                seed.len()
            )));
        }

        // Derive a synthetic public key from the seed using blake3
        let public_key: Vec<u8> = blake3::hash(seed).as_bytes().to_vec();

        let derivation_path = format!("m/44'/{}'/{}'", self.chain_id, 0);
        let address = Self::public_key_to_address_bytes(&public_key);
        let address_hex = Self::address_to_hex(&address);

        Ok(DerivedAddress {
            address,
            address_hex,
            public_key,
            derivation_path,
        })
    }

    /// Derive an ISA chain address from an existing public key.
    ///
    /// Produces the same result as `isa_chain_core::types::Address::from_public_key`:
    /// blake3 hash of the public key, taking the last 20 bytes (`[12..32]`).
    pub fn derive_from_public_key(&self, public_key: &[u8]) -> DerivedAddress {
        let address = Self::public_key_to_address_bytes(public_key);
        let address_hex = Self::address_to_hex(&address);

        DerivedAddress {
            address,
            address_hex,
            public_key: public_key.to_vec(),
            derivation_path: "direct".to_string(),
        }
    }

    /// Format a 20-byte address as a 0x-prefixed hex string.
    pub fn address_to_hex(address: &[u8; 20]) -> String {
        format!("0x{}", hex::encode(address))
    }

    /// Parse a 0x-prefixed hex string into a 20-byte address.
    pub fn hex_to_address(hex_str: &str) -> Result<[u8; 20], AddressDerivationError> {
        let stripped = hex_str
            .strip_prefix("0x")
            .or_else(|| hex_str.strip_prefix("0X"))
            .unwrap_or(hex_str);

        let bytes = hex::decode(stripped).map_err(|e| {
            AddressDerivationError::InvalidPublicKey(format!("invalid hex: {}", e))
        })?;

        if bytes.len() != 20 {
            return Err(AddressDerivationError::InvalidPublicKey(format!(
                "expected 20 bytes, got {}",
                bytes.len()
            )));
        }

        let mut address = [0u8; 20];
        address.copy_from_slice(&bytes);
        Ok(address)
    }

    /// Check whether `hex_str` is a valid ISA chain address (0x + 40 hex chars).
    pub fn validate_address(hex_str: &str) -> bool {
        let stripped = match hex_str.strip_prefix("0x").or_else(|| hex_str.strip_prefix("0X")) {
            Some(s) => s,
            None => return false,
        };

        if stripped.len() != 40 {
            return false;
        }

        stripped.chars().all(|c| c.is_ascii_hexdigit())
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /// Core address derivation: blake3(public_key)[12..32]
    ///
    /// Mirrors `isa_chain_core::types::Address::from_public_key` exactly.
    fn public_key_to_address_bytes(public_key: &[u8]) -> [u8; 20] {
        let hash = blake3::hash(public_key);
        let mut address = [0u8; 20];
        address.copy_from_slice(&hash.as_bytes()[12..32]);
        address
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use isa_chain_core::types::Address;

    // ── Helper ────────────────────────────────────────────────────────────────

    fn sample_public_key() -> Vec<u8> {
        // 33-byte compressed-style key (content is arbitrary for tests)
        (0u8..33).collect()
    }

    // ── Tests ─────────────────────────────────────────────────────────────────

    #[test]
    fn test_derive_from_public_key_matches_chain() {
        let pk = sample_public_key();
        let deriver = AddressDeriver::new(1);

        let derived = deriver.derive_from_public_key(&pk);
        let chain_addr = Address::from_public_key(&pk);

        assert_eq!(
            derived.address,
            *chain_addr.as_bytes(),
            "wallet address derivation must match chain Address::from_public_key"
        );
    }

    #[test]
    fn test_address_to_hex() {
        let mut addr = [0u8; 20];
        addr[0] = 0xab;
        addr[19] = 0xcd;

        let hex = AddressDeriver::address_to_hex(&addr);
        assert!(hex.starts_with("0x"), "hex must start with 0x");
        assert_eq!(hex.len(), 42, "0x + 40 hex chars = 42");
        assert!(hex.contains("ab"), "first byte must appear");
        assert!(hex.ends_with("cd"), "last byte must appear");
    }

    #[test]
    fn test_hex_to_address() {
        let input = [0x11u8; 20];
        let hex = AddressDeriver::address_to_hex(&input);
        let output = AddressDeriver::hex_to_address(&hex).expect("should parse back");
        assert_eq!(input, output);
    }

    #[test]
    fn test_validate_address_valid() {
        let pk = sample_public_key();
        let deriver = AddressDeriver::new(1);
        let derived = deriver.derive_from_public_key(&pk);

        assert!(
            AddressDeriver::validate_address(&derived.address_hex),
            "freshly derived address must be valid"
        );
    }

    #[test]
    fn test_validate_address_invalid() {
        assert!(!AddressDeriver::validate_address("not-an-address"));
        assert!(!AddressDeriver::validate_address("0x123")); // too short
        assert!(!AddressDeriver::validate_address("0xZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ")); // non-hex
        assert!(!AddressDeriver::validate_address("")); // empty
    }

    #[test]
    fn test_invalid_hex() {
        let result = AddressDeriver::hex_to_address("0xGGGG");
        assert!(result.is_err(), "invalid hex must return error");

        let result = AddressDeriver::hex_to_address("0x1234"); // too short
        assert!(result.is_err(), "too-short address must return error");
    }

    #[test]
    fn test_deterministic_derivation() {
        let seed = b"deterministic test seed 1234567890";
        let deriver = AddressDeriver::new(1);

        let a = deriver.derive_from_seed(seed).expect("first derivation");
        let b = deriver.derive_from_seed(seed).expect("second derivation");

        assert_eq!(a.address, b.address, "same seed must yield same address");
        assert_eq!(a.address_hex, b.address_hex);
        assert_eq!(a.public_key, b.public_key);
    }

    #[test]
    fn test_different_keys_different_addresses() {
        let deriver = AddressDeriver::new(1);

        let pk1: Vec<u8> = (0u8..32).collect();
        let pk2: Vec<u8> = (1u8..33).collect();

        let d1 = deriver.derive_from_public_key(&pk1);
        let d2 = deriver.derive_from_public_key(&pk2);

        assert_ne!(
            d1.address, d2.address,
            "different keys must produce different addresses"
        );
    }

    #[test]
    fn test_roundtrip_hex() {
        let pk = sample_public_key();
        let deriver = AddressDeriver::new(1);
        let derived = deriver.derive_from_public_key(&pk);

        let parsed = AddressDeriver::hex_to_address(&derived.address_hex)
            .expect("roundtrip must succeed");

        assert_eq!(derived.address, parsed, "hex roundtrip must be lossless");
    }

    #[test]
    fn test_seed_too_short_error() {
        let deriver = AddressDeriver::new(1);
        let result = deriver.derive_from_seed(b"tooshort");
        assert!(matches!(result, Err(AddressDerivationError::InvalidSeed(_))));
    }

    #[test]
    fn test_empty_seed_error() {
        let deriver = AddressDeriver::new(1);
        let result = deriver.derive_from_seed(b"");
        assert!(matches!(result, Err(AddressDerivationError::InvalidSeed(_))));
    }
}
