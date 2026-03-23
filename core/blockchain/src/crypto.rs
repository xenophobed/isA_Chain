use crate::types::*;

/// Errors from cryptographic operations
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("Invalid signature")]
    InvalidSignature,
    #[error("Invalid secret key")]
    InvalidSecretKey,
    #[error("Invalid public key")]
    InvalidPublicKey,
    #[error("Public key recovery failed")]
    RecoveryFailed,
}

/// Sign a message with a secp256k1 secret key.
///
/// The message is hashed with Blake3 before signing. Returns a `Signature`
/// with `r`, `s` (compact ECDSA components) and `v` (recovery ID 0 or 1).
pub fn sign_message(message: &[u8], secret_key: &[u8]) -> Result<Signature, CryptoError> {
    // Hash message with Blake3
    let hash = blake3::hash(message);
    let digest = hash.as_bytes();

    let secp = secp256k1::Secp256k1::new();
    let sk = secp256k1::SecretKey::from_slice(secret_key)
        .map_err(|_| CryptoError::InvalidSecretKey)?;

    let msg = secp256k1::Message::from_digest_slice(digest)
        .map_err(|_| CryptoError::InvalidSignature)?;

    let recoverable = secp.sign_ecdsa_recoverable(&msg, &sk);
    let (recovery_id, sig_bytes) = recoverable.serialize_compact();

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig_bytes[0..32]);
    s.copy_from_slice(&sig_bytes[32..64]);

    Ok(Signature::new(r, s, recovery_id.to_i32() as u8))
}

/// Verify a secp256k1 ECDSA signature against a message and a compressed public key.
///
/// The message is hashed with Blake3 before verification. Returns `true` only
/// when the signature is cryptographically valid for the given public key.
pub fn verify_signature(message: &[u8], signature: &Signature, public_key: &[u8]) -> bool {
    // Parse provided public key
    let pk = match secp256k1::PublicKey::from_slice(public_key) {
        Ok(pk) => pk,
        Err(_) => return false,
    };

    // Hash message with Blake3
    let hash = blake3::hash(message);
    let digest = hash.as_bytes();

    let secp = secp256k1::Secp256k1::new();
    let msg = match secp256k1::Message::from_digest_slice(digest) {
        Ok(m) => m,
        Err(_) => return false,
    };

    // Reconstruct compact signature bytes
    let mut sig_bytes = [0u8; 64];
    sig_bytes[0..32].copy_from_slice(&signature.r);
    sig_bytes[32..64].copy_from_slice(&signature.s);

    let recovery_id = match secp256k1::ecdsa::RecoveryId::from_i32(signature.v as i32) {
        Ok(id) => id,
        Err(_) => return false,
    };

    let recoverable = match secp256k1::ecdsa::RecoverableSignature::from_compact(
        &sig_bytes,
        recovery_id,
    ) {
        Ok(s) => s,
        Err(_) => return false,
    };

    // Convert to standard ECDSA signature for verify_ecdsa
    let ecdsa_sig = recoverable.to_standard();
    secp.verify_ecdsa(&msg, &ecdsa_sig, &pk).is_ok()
}

/// Generate a fresh secp256k1 key pair.
///
/// Returns `(secret_key_bytes, compressed_public_key_bytes)`.
pub fn generate_keypair() -> (Vec<u8>, Vec<u8>) {
    use rand::RngCore;
    let secp = secp256k1::Secp256k1::new();
    loop {
        let mut sk_bytes = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut sk_bytes);
        if let Ok(sk) = secp256k1::SecretKey::from_slice(&sk_bytes) {
            let pk = secp256k1::PublicKey::from_secret_key(&secp, &sk);
            return (sk_bytes.to_vec(), pk.serialize().to_vec());
        }
        // On the astronomically rare chance the random bytes are invalid,
        // loop and try again.
    }
}

/// Recover the compressed public key from a signed message.
///
/// Uses the `v` (recovery ID) field in `signature` to reconstruct the signer's
/// public key without needing it to be supplied out-of-band.
pub fn recover_public_key(message: &[u8], signature: &Signature) -> Result<Vec<u8>, CryptoError> {
    let hash = blake3::hash(message);
    let digest = hash.as_bytes();

    let secp = secp256k1::Secp256k1::new();
    let msg = secp256k1::Message::from_digest_slice(digest)
        .map_err(|_| CryptoError::RecoveryFailed)?;

    let mut sig_bytes = [0u8; 64];
    sig_bytes[0..32].copy_from_slice(&signature.r);
    sig_bytes[32..64].copy_from_slice(&signature.s);

    let recovery_id = secp256k1::ecdsa::RecoveryId::from_i32(signature.v as i32)
        .map_err(|_| CryptoError::InvalidSignature)?;

    let recoverable = secp256k1::ecdsa::RecoverableSignature::from_compact(&sig_bytes, recovery_id)
        .map_err(|_| CryptoError::InvalidSignature)?;

    let pk = secp
        .recover_ecdsa(&msg, &recoverable)
        .map_err(|_| CryptoError::RecoveryFailed)?;

    Ok(pk.serialize().to_vec())
}

/// Cryptographic utilities
pub struct CryptoUtils;

impl CryptoUtils {
    /// Generate a random private key
    pub fn generate_private_key() -> [u8; 32] {
        use rand::RngCore;
        let mut key = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    /// Derive public key from private key
    pub fn derive_public_key(private_key: &[u8; 32]) -> Result<[u8; 33], &'static str> {
        let secp = secp256k1::Secp256k1::new();
        let secret_key = secp256k1::SecretKey::from_slice(private_key)
            .map_err(|_| "Invalid private key")?;
        let public_key = secp256k1::PublicKey::from_secret_key(&secp, &secret_key);
        Ok(public_key.serialize())
    }

    /// Derive address from public key
    pub fn derive_address(public_key: &[u8]) -> Address {
        Address::from_public_key(public_key)
    }

    /// Hash data using Blake3
    pub fn hash_blake3(data: &[u8]) -> Hash {
        Hash::hash_data(data)
    }

    /// Hash data using SHA256
    pub fn hash_sha256(data: &[u8]) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Hash data using Keccak256 (Ethereum compatible)
    pub fn hash_keccak256(data: &[u8]) -> [u8; 32] {
        use sha3::{Digest, Keccak256};
        let mut hasher = Keccak256::new();
        hasher.update(data);
        hasher.finalize().into()
    }

    /// Generate random bytes
    pub fn random_bytes(length: usize) -> Vec<u8> {
        use rand::RngCore;
        let mut bytes = vec![0u8; length];
        rand::thread_rng().fill_bytes(&mut bytes);
        bytes
    }

    /// Verify ECDSA signature (delegates to the real implementation)
    pub fn verify_signature(
        message: &[u8],
        signature: &Signature,
        public_key: &[u8],
    ) -> bool {
        verify_signature(message, signature, public_key)
    }
}

/// Key pair for cryptographic operations
#[derive(Debug, Clone)]
pub struct KeyPair {
    pub private_key: [u8; 32],
    pub public_key: [u8; 33],
    pub address: Address,
}

impl KeyPair {
    /// Generate a new random key pair
    pub fn generate() -> Result<Self, &'static str> {
        let private_key = CryptoUtils::generate_private_key();
        let public_key = CryptoUtils::derive_public_key(&private_key)?;
        let address = CryptoUtils::derive_address(&public_key);

        Ok(KeyPair {
            private_key,
            public_key,
            address,
        })
    }

    /// Create key pair from existing private key
    pub fn from_private_key(private_key: [u8; 32]) -> Result<Self, &'static str> {
        let public_key = CryptoUtils::derive_public_key(&private_key)?;
        let address = CryptoUtils::derive_address(&public_key);

        Ok(KeyPair {
            private_key,
            public_key,
            address,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── existing tests ──────────────────────────────────────────────────────

    #[test]
    fn test_key_generation() {
        let keypair = KeyPair::generate().unwrap();
        assert_ne!(keypair.private_key, [0u8; 32]);
        assert_ne!(keypair.public_key, [0u8; 33]);
        assert_ne!(keypair.address, Address::ZERO);
    }

    #[test]
    fn test_address_derivation() {
        let private_key = [1u8; 32];
        let public_key = CryptoUtils::derive_public_key(&private_key).unwrap();
        let address1 = CryptoUtils::derive_address(&public_key);
        let address2 = CryptoUtils::derive_address(&public_key);

        // Should be deterministic
        assert_eq!(address1, address2);
    }

    #[test]
    fn test_hashing() {
        let data = b"hello world";
        let hash1 = CryptoUtils::hash_blake3(data);
        let hash2 = CryptoUtils::hash_blake3(data);

        // Should be deterministic
        assert_eq!(hash1, hash2);

        // Different data should produce different hash
        let hash3 = CryptoUtils::hash_blake3(b"hello world!");
        assert_ne!(hash1, hash3);
    }

    // ── new tests for issue #73 ─────────────────────────────────────────────

    #[test]
    fn test_sign_and_verify() {
        let (sk, pk) = generate_keypair();
        let message = b"hello isA_Chain";

        let sig = sign_message(message, &sk).expect("sign should succeed");
        assert!(
            verify_signature(message, &sig, &pk),
            "verification of a valid signature must return true"
        );
    }

    #[test]
    fn test_verify_wrong_key_fails() {
        let (sk, _pk) = generate_keypair();
        let (_sk2, pk2) = generate_keypair();
        let message = b"hello isA_Chain";

        let sig = sign_message(message, &sk).unwrap();
        assert!(
            !verify_signature(message, &sig, &pk2),
            "verification with a different public key must return false"
        );
    }

    #[test]
    fn test_verify_tampered_message_fails() {
        let (sk, pk) = generate_keypair();
        let message = b"original message";
        let tampered = b"tampered message";

        let sig = sign_message(message, &sk).unwrap();
        assert!(
            !verify_signature(tampered, &sig, &pk),
            "verification of a tampered message must return false"
        );
    }

    #[test]
    fn test_verify_invalid_signature_fails() {
        let (_sk, pk) = generate_keypair();
        let message = b"some message";

        // A signature with all-zero r and s is invalid
        let bad_sig = Signature::new([0u8; 32], [0u8; 32], 0);
        assert!(
            !verify_signature(message, &bad_sig, &pk),
            "verification of an all-zero signature must return false"
        );
    }

    #[test]
    fn test_generate_keypair() {
        let (sk, pk) = generate_keypair();
        assert_eq!(sk.len(), 32, "secret key must be 32 bytes");
        assert_eq!(pk.len(), 33, "compressed public key must be 33 bytes");
        // Public key must start with 0x02 or 0x03 (compressed secp256k1 prefix)
        assert!(pk[0] == 0x02 || pk[0] == 0x03, "invalid compressed public key prefix");
    }

    #[test]
    fn test_recover_public_key() {
        let (sk, pk) = generate_keypair();
        let message = b"recover me";

        let sig = sign_message(message, &sk).unwrap();
        let recovered = recover_public_key(message, &sig).expect("recovery must succeed");

        assert_eq!(
            recovered, pk,
            "recovered public key must match the original"
        );
    }

    #[test]
    fn test_sign_deterministic() {
        // secp256k1 RFC 6979 — same key + message always produces the same signature
        let (sk, _pk) = generate_keypair();
        let message = b"determinism test";

        let sig1 = sign_message(message, &sk).unwrap();
        let sig2 = sign_message(message, &sk).unwrap();

        assert_eq!(sig1.r, sig2.r, "r components must be identical");
        assert_eq!(sig1.s, sig2.s, "s components must be identical");
        assert_eq!(sig1.v, sig2.v, "recovery ids must be identical");
    }
}
