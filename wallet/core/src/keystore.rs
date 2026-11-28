use crate::crypto::EncryptionKey;
use crate::error::WalletError;
use crate::mnemonic::Mnemonic;
use bip32::{DerivationPath, XPrv};
use isa_chain_core::crypto::KeyPair;
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeystoreType {
    Encrypted,
    PlainText, // Reserved for testing. Persisting unencrypted secrets is unsupported.
}

const HEADER: &[u8] = b"ISAK\x01";
const SALT_SIZE: usize = 16;
const MNEMONIC: u8 = 1;
const PRIVATE_KEY: u8 = 2;

#[derive(Clone)]
enum RootKey {
    Hierarchical(XPrv),
    Single(Zeroizing<[u8; 32]>),
}

pub struct Keystore {
    root: RootKey,
    encrypted_data: Vec<u8>,
}

impl std::fmt::Debug for Keystore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keystore").finish_non_exhaustive()
    }
}

impl Keystore {
    pub fn from_mnemonic(
        mnemonic: Mnemonic,
        password: &str,
        mode: KeystoreType,
    ) -> Result<Self, WalletError> {
        let phrase = Zeroizing::new(mnemonic.phrase());
        Self::create(MNEMONIC, phrase.as_bytes(), password, mode)
    }

    pub fn from_private_key(
        private_key: [u8; 32],
        password: &str,
        mode: KeystoreType,
    ) -> Result<Self, WalletError> {
        let key = Zeroizing::new(private_key);
        Self::create(PRIVATE_KEY, &*key, password, mode)
    }

    fn create(
        kind: u8,
        secret: &[u8],
        password: &str,
        mode: KeystoreType,
    ) -> Result<Self, WalletError> {
        if matches!(mode, KeystoreType::PlainText) {
            return Err(WalletError::UnsupportedOperation(
                "Plaintext keystores are unsupported".into(),
            ));
        }
        let root = Self::root_from_secret(kind, secret)?;
        let mut payload = Zeroizing::new(Vec::with_capacity(secret.len() + 1));
        payload.push(kind);
        payload.extend_from_slice(secret);
        Ok(Self {
            root,
            encrypted_data: Self::seal(&payload, password)?,
        })
    }

    fn root_from_secret(kind: u8, secret: &[u8]) -> Result<RootKey, WalletError> {
        match kind {
            MNEMONIC => {
                let mnemonic = Self::parse_mnemonic(secret)?;
                // Wallet passwords protect storage. They are independent of the BIP39 seed.
                let seed = Zeroizing::new(mnemonic.to_seed_bytes(""));
                XPrv::new(&*seed)
                    .map(RootKey::Hierarchical)
                    .map_err(|e| WalletError::CryptoError(e.to_string()))
            }
            PRIVATE_KEY => {
                let key: Zeroizing<[u8; 32]> = Zeroizing::new(
                    secret
                        .try_into()
                        .map_err(|_| WalletError::KeystoreError("Invalid key length".into()))?,
                );
                secp256k1::SecretKey::from_slice(&*key)
                    .map_err(|_| WalletError::KeystoreError("Invalid private key".into()))?;
                Ok(RootKey::Single(key))
            }
            _ => Err(WalletError::KeystoreError(
                "Unknown key material type".into(),
            )),
        }
    }

    fn parse_mnemonic(bytes: &[u8]) -> Result<Mnemonic, WalletError> {
        let phrase = std::str::from_utf8(bytes)
            .map_err(|_| WalletError::KeystoreError("Invalid recovery phrase encoding".into()))?;
        Mnemonic::from_phrase(phrase)
    }

    fn seal(payload: &[u8], password: &str) -> Result<Vec<u8>, WalletError> {
        let mut salt = [0; SALT_SIZE];
        OsRng.fill_bytes(&mut salt);
        let key = EncryptionKey::derive_from_password(password, &salt)?;
        let mut bytes = HEADER.to_vec();
        bytes.extend(salt);
        bytes.extend(key.encrypt(payload)?);
        Ok(bytes)
    }

    fn open(bytes: &[u8], password: &str) -> Result<Zeroizing<Vec<u8>>, WalletError> {
        let data = bytes.strip_prefix(HEADER).ok_or_else(|| {
            WalletError::KeystoreError("Unsupported or empty legacy keystore".into())
        })?;
        if data.len() < SALT_SIZE {
            return Err(WalletError::KeystoreError("Truncated keystore".into()));
        }
        let (salt, ciphertext) = data.split_at(SALT_SIZE);
        let key = EncryptionKey::derive_from_password(password, salt)?;
        key.decrypt(ciphertext).map(Zeroizing::new)
    }

    pub fn load(data: &[u8], password: &str) -> Result<Self, WalletError> {
        let payload = Self::open(data, password)?;
        let (kind, secret) = payload
            .split_first()
            .ok_or_else(|| WalletError::KeystoreError("Empty key material".into()))?;
        Ok(Self {
            root: Self::root_from_secret(*kind, secret)?,
            encrypted_data: data.to_vec(),
        })
    }

    pub fn derive_key(&self, path: &str) -> Result<KeyPair, WalletError> {
        let key = match &self.root {
            RootKey::Hierarchical(root) => {
                let path: DerivationPath = path
                    .parse()
                    .map_err(|e: bip32::Error| WalletError::CryptoError(e.to_string()))?;
                let child = path
                    .iter()
                    .try_fold(root.clone(), |key, child| key.derive_child(child))
                    .map_err(|e| WalletError::CryptoError(e.to_string()))?;
                child.to_bytes()
            }
            RootKey::Single(key) if path == "m" => **key,
            RootKey::Single(_) => {
                return Err(WalletError::UnsupportedOperation(
                    "Single-key wallets have no child accounts".into(),
                ))
            }
        };
        KeyPair::from_private_key(key).map_err(|e| WalletError::CryptoError(e.into()))
    }

    pub fn get_private_key(&self, path: &str, password: &str) -> Result<[u8; 32], WalletError> {
        self.verify_password(password)?;
        Ok(self.derive_key(path)?.private_key)
    }

    pub fn get_master_private_key(&self, password: &str) -> Result<[u8; 32], WalletError> {
        self.get_private_key("m", password)
    }

    pub fn export_mnemonic(&self, password: &str) -> Result<Mnemonic, WalletError> {
        let payload = Self::open(&self.encrypted_data, password)?;
        match payload.split_first() {
            Some((&MNEMONIC, phrase)) => Self::parse_mnemonic(phrase),
            _ => Err(WalletError::UnsupportedOperation(
                "Single-key wallets have no recovery phrase".into(),
            )),
        }
    }

    pub fn verify_password(&self, password: &str) -> Result<(), WalletError> {
        Self::open(&self.encrypted_data, password).map(|_| ())
    }

    pub(crate) fn reencrypted(&self, old: &str, new: &str) -> Result<Self, WalletError> {
        Ok(Self {
            root: self.root.clone(),
            encrypted_data: Self::seal(&Self::open(&self.encrypted_data, old)?, new)?,
        })
    }

    pub fn change_password(&mut self, old: &str, new: &str) -> Result<(), WalletError> {
        *self = self.reencrypted(old, new)?;
        Ok(())
    }

    pub fn export(&self) -> Result<Vec<u8>, WalletError> {
        Ok(self.encrypted_data.clone())
    }
}
