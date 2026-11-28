use isa_chain_core::crypto::KeyPair;
use isa_chain_core::types::Address;
use isa_wallet_core::{
    Keystore, KeystoreType, Mnemonic, NetworkConfig, Wallet, WalletError, WalletExport,
    WalletParams, WalletType,
};

// Wallet::create generated these UUIDs in this test. Remove its private sled
// directory even on a failed assertion; public delete only clears owned records.
struct TestWalletDir(std::path::PathBuf);

impl TestWalletDir {
    fn created(wallet: &Wallet) -> Self {
        let id = uuid::Uuid::parse_str(&wallet.id).unwrap();
        Self(std::path::Path::new("wallets").join(id.to_string()))
    }
}

impl Drop for TestWalletDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn wallet_params(mnemonic: Option<String>, password: &str) -> WalletParams {
    WalletParams {
        name: "Disposable recovery test".into(),
        password: password.into(),
        wallet_type: WalletType::Hierarchical,
        network: NetworkConfig {
            chain_id: 1,
            network_name: "offline".into(),
            rpc_url: String::new(),
            explorer_url: String::new(),
            currency_symbol: "ISA".into(),
            currency_decimals: 18,
        },
        mnemonic,
        entropy_length: Some(128),
    }
}

#[tokio::test]
async fn restores_and_reopens_the_same_accounts() {
    let mut original = Wallet::create(wallet_params(None, "first password"))
        .await
        .unwrap();
    let _original_dir = TestWalletDir::created(&original);
    let first = original.get_primary_account().unwrap().address;
    let second = original.derive_next_account().await.unwrap();
    assert_eq!(second.derivation_path, "m/44'/60'/0'/0/1");
    assert!(first != second.address);
    let WalletExport::Mnemonic(phrase) = original.export("first password").unwrap() else {
        panic!("hierarchical wallet must export its recovery phrase");
    };
    let mut restored = Wallet::create(wallet_params(Some(phrase), "restore password"))
        .await
        .unwrap();
    let _restored_dir = TestWalletDir::created(&restored);
    assert!(restored.get_primary_account().unwrap().address == first);
    assert!(restored.derive_next_account().await.unwrap().address == second.address);

    let id = original.id.clone();
    original
        .update_account_balance(second.address, 17)
        .await
        .unwrap();
    original
        .change_password("first password", "rotated password")
        .await
        .unwrap();
    drop(original);
    assert!(matches!(
        Wallet::load(&id, "first password").await,
        Err(WalletError::InvalidPassword)
    ));
    let mut reopened = Wallet::load(&id, "rotated password").await.unwrap();
    assert!(reopened.get_primary_account().unwrap().address == first);
    assert_eq!(reopened.get_account(&second.address).unwrap().balance, 17);
    let third = reopened.derive_next_account().await.unwrap();
    assert_eq!(third.derivation_path, "m/44'/60'/0'/0/2");
    assert!(third.address == restored.derive_next_account().await.unwrap().address);
    let signed = reopened
        .create_transfer(third.address, first, 1, 21_000, 1, "rotated password")
        .await
        .unwrap();
    assert!(signed.verify().is_ok());
    let params = isa_wallet_core::SigningParams {
        from: first,
        password: "rotated password".into(),
        transaction: signed,
    };
    assert!(!format!("{params:?}").contains("rotated password"));
    assert!(matches!(
        reopened.sign_transaction(params).await,
        Err(WalletError::SigningError(_))
    ));
    reopened.delete().await.unwrap();
    assert!(Wallet::load(&id, "rotated password").await.is_err());
    restored.delete().await.unwrap();
}

#[test]
fn exported_keystore_is_authenticated_and_legacy_empty_exports_fail() {
    let mnemonic = Mnemonic::generate(128).unwrap();
    let phrase = mnemonic.phrase();
    let keystore = Keystore::from_mnemonic(mnemonic, "password", KeystoreType::Encrypted).unwrap();
    let mut bytes = keystore.export().unwrap();
    assert!(!bytes.windows(phrase.len()).any(|w| w == phrase.as_bytes()));
    assert!(Keystore::load(&[], "password").is_err());
    let recovered = Keystore::load(&bytes, "password").unwrap();
    assert!(recovered.export_mnemonic("password").unwrap().phrase() == phrase);
    assert!(matches!(
        recovered.export_mnemonic("incorrect"),
        Err(WalletError::InvalidPassword)
    ));
    assert!(recovered.derive_key("invalid").is_err());
    assert!(!format!("{:?}", recovered.derive_key("m").unwrap()).contains("private_key"));
    let end = bytes.last_mut().unwrap();
    *end ^= 1;
    assert!(Keystore::load(&bytes, "password").is_err());
}

#[tokio::test]
async fn single_key_wallet_reopens_its_only_account() {
    let mut params = wallet_params(None, "single password");
    params.wallet_type = WalletType::SingleKey;
    let mut wallet = Wallet::create(params).await.unwrap();
    let _wallet_dir = TestWalletDir::created(&wallet);
    let account = wallet.get_primary_account().unwrap().address;
    let WalletExport::PrivateKey(key) = wallet.export("single password").unwrap() else {
        panic!("single key wallet must export its own key");
    };
    let key = hex::decode(key).unwrap().try_into().unwrap();
    let expected = KeyPair::from_private_key(key).unwrap();
    assert!(account == expected.address && account != Address::ZERO);
    assert!(wallet.derive_next_account().await.is_err());
    let id = wallet.id.clone();
    drop(wallet);
    let loaded = Wallet::load(&id, "single password").await.unwrap();
    assert!(loaded.get_primary_account().unwrap().address == account);
    loaded.delete().await.unwrap();
}

#[test]
fn wallet_debug_omits_recovery_material_and_passwords() {
    let phrase = Mnemonic::generate(128).unwrap();
    let text = phrase.phrase();
    let password = uuid::Uuid::new_v4().to_string();
    let params = wallet_params(Some(text.clone()), &password);
    let export = WalletExport::Mnemonic(text.clone());
    assert!(!format!("{params:?}").contains(&password));
    assert!(!format!("{params:?}").contains(&text));
    assert!(!format!("{export:?}").contains(&text));
    assert!(!format!("{phrase:?}").contains("words"));
}

#[tokio::test]
async fn reject_unowned_wallet_paths() {
    assert!(matches!(
        Wallet::load("../outside", "password").await,
        Err(WalletError::StorageError(_))
    ));
    let unknown = uuid::Uuid::new_v4().to_string();
    assert!(Wallet::load(&unknown, "password").await.is_err());
    assert!(!std::path::Path::new("wallets").join(&unknown).exists());
    assert!(Wallet::load("/tmp/unowned", "password").await.is_err());
    #[cfg(unix)]
    {
        let wallet = Wallet::create(wallet_params(None, "disposable"))
            .await
            .unwrap();
        let directory = TestWalletDir::created(&wallet);
        drop(wallet);
        let alias = std::path::Path::new("wallets").join(&unknown);
        std::os::unix::fs::symlink(directory.0.canonicalize().unwrap(), &alias).unwrap();
        let result = Wallet::load(&unknown, "disposable").await;
        std::fs::remove_file(alias).unwrap();
        assert!(matches!(result, Err(WalletError::StorageError(_))));
    }
}

#[tokio::test]
async fn reject_mismatched_stored_addresses_without_substituting_new_ones() {
    use isa_wallet_core::storage::{Storage, WalletStorage};
    let wallet = Wallet::create(wallet_params(None, "password"))
        .await
        .unwrap();
    let directory = TestWalletDir::created(&wallet);
    let id = wallet.id.clone();
    drop(wallet);
    let storage = WalletStorage::new(directory.0.to_str().unwrap()).unwrap();
    let mut accounts = storage.load_accounts().await.unwrap();
    let mut entry = accounts.values().next().unwrap().clone();
    accounts.clear();
    entry.address = Address::ZERO;
    accounts.insert(entry.address, entry);
    storage.save_accounts(&accounts).await.unwrap();
    drop(storage);
    assert!(matches!(
        Wallet::load(&id, "password").await,
        Err(WalletError::StorageError(_))
    ));
    let storage = WalletStorage::new(directory.0.to_str().unwrap()).unwrap();
    assert!(storage
        .load_accounts()
        .await
        .unwrap()
        .contains_key(&Address::ZERO));
}
