use crate::error::WalletError;
use aes_gcm::{
    aead::{Aead, AeadInPlace, KeyInit, OsRng, Payload},
    Aes256Gcm, Nonce,
};
use rand::RngCore;
use ring::pbkdf2;
use std::num::NonZeroU32;
use zeroize::{ZeroizeOnDrop, Zeroizing};

// Version 1 fixes the KDF and AEAD parameters for all stored keystores.
const CONTEXT: &[u8] = b"isA wallet keystore v1";
const NONCE_SIZE: usize = 12;

#[derive(ZeroizeOnDrop)]
pub struct EncryptionKey {
    key: [u8; 32],
}

impl std::fmt::Debug for EncryptionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptionKey").finish_non_exhaustive()
    }
}

impl EncryptionKey {
    pub fn derive_from_password(password: &str, salt: &[u8]) -> Result<Self, WalletError> {
        if salt.len() < 16 {
            return Err(WalletError::KeystoreError(
                "Keystore salt is too short".into(),
            ));
        }
        let mut key = Self { key: [0; 32] };
        pbkdf2::derive(
            pbkdf2::PBKDF2_HMAC_SHA256,
            NonZeroU32::new(600_000).unwrap(),
            salt,
            password.as_bytes(),
            &mut key.key,
        );
        Ok(key)
    }

    pub fn encrypt(&self, data: &[u8]) -> Result<Vec<u8>, WalletError> {
        let mut nonce = [0; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce);
        let ciphertext = Aes256Gcm::new((&self.key).into())
            .encrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: data,
                    aad: CONTEXT,
                },
            )
            .map_err(|_| WalletError::CryptoError("Keystore encryption failed".into()))?;
        let mut result = nonce.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, WalletError> {
        if data.len() < NONCE_SIZE + 16 {
            return Err(WalletError::KeystoreError(
                "Truncated encrypted keystore".into(),
            ));
        }
        let (nonce, ciphertext) = data.split_at(NONCE_SIZE);
        let mut result = Zeroizing::new(ciphertext.to_vec());
        Aes256Gcm::new((&self.key).into())
            .decrypt_in_place(Nonce::from_slice(nonce), CONTEXT, &mut *result)
            .map_err(|_| WalletError::InvalidPassword)?;
        Ok(std::mem::take(&mut *result))
    }
}
