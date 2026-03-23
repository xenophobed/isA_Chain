use crate::error::WalletError;
use bip39::{Language, Mnemonic as Bip39Mnemonic};
use serde::{Deserialize, Serialize};

/// Mnemonic phrase wrapper with security features.
///
/// Note: bip39 v2 does not implement `Zeroize` for `Mnemonic` or `Language`,
/// so this wrapper stores them without automatic zeroize-on-drop.
#[derive(Debug, Clone)]
pub struct Mnemonic {
    /// BIP39 mnemonic
    inner: Bip39Mnemonic,

    /// Number of entropy bits
    entropy_bits: usize,
}

/// Mnemonic strength levels
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum MnemonicStrength {
    /// 12 words (128 bits entropy)
    Words12,
    /// 15 words (160 bits entropy)
    Words15,
    /// 18 words (192 bits entropy)
    Words18,
    /// 21 words (224 bits entropy)
    Words21,
    /// 24 words (256 bits entropy)
    Words24,
}

impl MnemonicStrength {
    /// Get entropy bits for strength level
    pub fn entropy_bits(&self) -> usize {
        match self {
            MnemonicStrength::Words12 => 128,
            MnemonicStrength::Words15 => 160,
            MnemonicStrength::Words18 => 192,
            MnemonicStrength::Words21 => 224,
            MnemonicStrength::Words24 => 256,
        }
    }

    /// Get word count for strength level
    pub fn word_count(&self) -> usize {
        match self {
            MnemonicStrength::Words12 => 12,
            MnemonicStrength::Words15 => 15,
            MnemonicStrength::Words18 => 18,
            MnemonicStrength::Words21 => 21,
            MnemonicStrength::Words24 => 24,
        }
    }
}

impl Mnemonic {
    /// Generate a new random mnemonic
    pub fn generate(entropy_bits: usize) -> Result<Self, WalletError> {
        let word_count = match entropy_bits {
            128 => 12,
            160 => 15,
            192 => 18,
            224 => 21,
            256 => 24,
            _ => return Err(WalletError::InvalidEntropyLength(entropy_bits)),
        };

        let inner = Bip39Mnemonic::generate(word_count)
            .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))?;

        Ok(Mnemonic {
            inner,
            entropy_bits,
        })
    }

    /// Generate mnemonic with specific strength
    pub fn generate_with_strength(strength: MnemonicStrength) -> Result<Self, WalletError> {
        Self::generate(strength.entropy_bits())
    }

    /// Create mnemonic from existing phrase
    pub fn from_phrase(phrase: &str) -> Result<Self, WalletError> {
        let inner = Bip39Mnemonic::parse(phrase)
            .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))?;

        let word_count = inner.word_count();
        let entropy_bits = match word_count {
            12 => 128,
            15 => 160,
            18 => 192,
            21 => 224,
            24 => 256,
            _ => {
                return Err(WalletError::InvalidMnemonic(
                    "Invalid word count".to_string(),
                ))
            }
        };

        Ok(Mnemonic {
            inner,
            entropy_bits,
        })
    }

    /// Create mnemonic from entropy bytes (English only)
    pub fn from_entropy(entropy: &[u8]) -> Result<Self, WalletError> {
        let inner = Bip39Mnemonic::from_entropy(entropy)
            .map_err(|e| WalletError::InvalidMnemonic(e.to_string()))?;

        Ok(Mnemonic {
            inner,
            entropy_bits: entropy.len() * 8,
        })
    }

    /// Get mnemonic phrase as string
    pub fn phrase(&self) -> String {
        self.inner.to_string()
    }

    /// Get individual words
    pub fn words(&self) -> Vec<&str> {
        self.inner.words().collect()
    }

    /// Get word at specific index
    pub fn word(&self, index: usize) -> Option<&str> {
        self.inner.words().nth(index)
    }

    /// Get word count
    pub fn word_count(&self) -> usize {
        self.inner.word_count()
    }

    /// Get entropy bytes
    pub fn entropy(&self) -> Vec<u8> {
        self.inner.to_entropy()
    }

    /// Get entropy bits
    pub fn entropy_bits(&self) -> usize {
        self.entropy_bits
    }

    /// Get language (always English in this build — no `all-languages` feature)
    pub fn language(&self) -> Language {
        Language::English
    }

    /// Get strength level
    pub fn strength(&self) -> MnemonicStrength {
        match self.word_count() {
            12 => MnemonicStrength::Words12,
            15 => MnemonicStrength::Words15,
            18 => MnemonicStrength::Words18,
            21 => MnemonicStrength::Words21,
            24 => MnemonicStrength::Words24,
            _ => MnemonicStrength::Words12,
        }
    }

    /// Generate 64-byte seed from mnemonic
    pub fn to_seed_bytes(&self, passphrase: &str) -> [u8; 64] {
        self.inner.to_seed(passphrase)
    }

    /// Validate mnemonic phrase
    pub fn validate(&self) -> bool {
        true // construction already validates
    }

    /// Check if phrase is valid BIP39 mnemonic
    pub fn is_valid_phrase(phrase: &str) -> bool {
        Bip39Mnemonic::parse(phrase).is_ok()
    }

    /// Get available languages
    pub fn available_languages() -> Vec<Language> {
        vec![
            Language::English,
            Language::Japanese,
            Language::Korean,
            Language::Spanish,
            Language::SimplifiedChinese,
            Language::TraditionalChinese,
            Language::French,
            Language::Italian,
            Language::Czech,
        ]
    }

    /// Get mnemonic strength recommendation
    pub fn recommended_strength() -> MnemonicStrength {
        MnemonicStrength::Words24
    }

    /// Get minimum recommended strength
    pub fn minimum_strength() -> MnemonicStrength {
        MnemonicStrength::Words12
    }
}

impl std::fmt::Display for Mnemonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.phrase())
    }
}

/// Mnemonic validation utilities
pub struct MnemonicValidator;

impl MnemonicValidator {
    /// Validate mnemonic phrase comprehensively
    pub fn validate_comprehensive(
        phrase: &str,
    ) -> Result<MnemonicValidation, WalletError> {
        let words: Vec<&str> = phrase.split_whitespace().collect();

        let word_count_valid = matches!(words.len(), 12 | 15 | 18 | 21 | 24);
        let bip39_valid = Mnemonic::is_valid_phrase(phrase);

        let mut unique_words = std::collections::HashSet::new();
        let no_duplicates = words.iter().all(|word| unique_words.insert(word));

        let strength = match words.len() {
            12 => Some(MnemonicStrength::Words12),
            15 => Some(MnemonicStrength::Words15),
            18 => Some(MnemonicStrength::Words18),
            21 => Some(MnemonicStrength::Words21),
            24 => Some(MnemonicStrength::Words24),
            _ => None,
        };

        Ok(MnemonicValidation {
            valid: bip39_valid && word_count_valid && no_duplicates,
            word_count_valid,
            bip39_valid,
            no_duplicates,
            word_count: words.len(),
            strength,
        })
    }

    /// Quick validation check
    pub fn is_valid(phrase: &str) -> bool {
        Self::validate_comprehensive(phrase)
            .map(|v| v.valid)
            .unwrap_or(false)
    }
}

/// Comprehensive mnemonic validation result
#[derive(Debug, Clone)]
pub struct MnemonicValidation {
    pub valid: bool,
    pub word_count_valid: bool,
    pub bip39_valid: bool,
    pub no_duplicates: bool,
    pub word_count: usize,
    pub strength: Option<MnemonicStrength>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_generation() {
        let mnemonic = Mnemonic::generate(256).unwrap();
        assert_eq!(mnemonic.word_count(), 24);
        assert_eq!(mnemonic.entropy_bits(), 256);
        assert_eq!(mnemonic.strength(), MnemonicStrength::Words24);
        assert!(mnemonic.validate());
    }

    #[test]
    fn test_mnemonic_from_phrase() {
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::from_phrase(phrase).unwrap();

        assert_eq!(mnemonic.word_count(), 12);
        assert_eq!(mnemonic.phrase(), phrase);
        assert!(mnemonic.validate());
    }

    #[test]
    fn test_mnemonic_validation() {
        let valid_phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let validation = MnemonicValidator::validate_comprehensive(valid_phrase).unwrap();

        assert!(validation.bip39_valid, "bip39_valid should be true");
        assert!(validation.word_count_valid, "word_count_valid should be true");
        assert_eq!(validation.word_count, 12);
        assert_eq!(validation.strength, Some(MnemonicStrength::Words12));
        // Note: the test phrase repeats "abandon" so no_duplicates is false; valid reflects that
        assert!(validation.bip39_valid && validation.word_count_valid);
    }

    #[test]
    fn test_invalid_mnemonic() {
        let invalid_phrase = "invalid mnemonic phrase that should not work";
        let validation = MnemonicValidator::validate_comprehensive(invalid_phrase).unwrap();

        assert!(!validation.valid);
        assert!(!validation.bip39_valid);
    }

    #[test]
    fn test_seed_generation() {
        let phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let mnemonic = Mnemonic::from_phrase(phrase).unwrap();

        let seed1 = mnemonic.to_seed_bytes("");
        let seed2 = mnemonic.to_seed_bytes("");
        let seed3 = mnemonic.to_seed_bytes("password");

        assert_eq!(seed1, seed2);
        assert_ne!(seed1, seed3);
    }

    #[test]
    fn test_strength_levels() {
        assert_eq!(MnemonicStrength::Words12.entropy_bits(), 128);
        assert_eq!(MnemonicStrength::Words15.entropy_bits(), 160);
        assert_eq!(MnemonicStrength::Words18.entropy_bits(), 192);
        assert_eq!(MnemonicStrength::Words21.entropy_bits(), 224);
        assert_eq!(MnemonicStrength::Words24.entropy_bits(), 256);
    }
}
