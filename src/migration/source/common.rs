use crate::{
    migration::source::json_data::{KeyFileV2, CryptoTypeV2},
    storage::vault::VaultStorage,
    core::chains::Blockchain,
    structs::{
        pk::{PrivateKeyHolder, PrivateKeyType, EthereumPk3},
        crypto::Encrypted,
        wallet::{WalletAccount, PKType, Wallet},
        types::HasUuid,
        seed::{SeedSource, Seed, LedgerSource, HDPathFingerprint, SeedRef}
    }
};
use uuid::Uuid;
use std::convert::TryFrom;

fn extract_label(kf: &KeyFileV2) -> Option<String> {
    let mut result = String::new();
    match &kf.name {
        Some(name) => result.push_str(name.as_str()),
        None => {}
    }
    match &kf.visible {
        Some(visible) if !visible => {
            if !result.is_empty() {
                result.push(' ');
            }
            result.push_str("(legacy hidden)")
        },
        _ => {}
    }
    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

// Creates Private Key and Wallet with that single key
pub fn add_to_vault(blockchain: Blockchain, vault: &VaultStorage, kf: &KeyFileV2) -> Result<Uuid, String> {
    let account = match &kf.crypto {
        CryptoTypeV2::Core(data) => {
            let pk = PrivateKeyHolder {
                id: Uuid::new_v4(),
                pk: PrivateKeyType::EthereumPk(
                    EthereumPk3 {
                        address: kf.address,
                        key: Encrypted::try_from(data).map_err(|e| "Failed to convert encrypted Private Key")?
                    }
                )
            };
            let pk_id = pk.get_id();
            vault.keys().add(pk).map_err(|e| "Failed to add converted Private Key to the Vault")?;
            WalletAccount {
                blockchain,
                address: kf.address,
                key: PKType::PrivateKeyRef(pk_id)
            }
        },
        CryptoTypeV2::HdWallet(data) => {
            let seeds = vault.seeds();
            //during migration consider that user has only one ledger
            let existing = seeds.list_entries()
                .map_err(|_| "Failed to read list of current Seeds".to_string())?
                .iter()
                .find(|s| match s.source {
                    SeedSource::Ledger(_) => true,
                    _ => false
                })
                .cloned();

            let seed_id = match &existing {
                Some(seed) => seed.id.clone(),
                None => {
                    let fingerprints = match kf.address {
                        Some(address) => {
                            let f = HDPathFingerprint::from_address(data.hd_path.clone(), &address);
                            vec![f]
                        },
                        None => Vec::new()
                    };
                    let seed = Seed {
                        id: Uuid::new_v4(),
                        source: SeedSource::Ledger(
                            LedgerSource { fingerprints }
                        )
                    };
                    let id = seed.id.clone();
                    seeds.add(seed).map_err(|e| "Failed to add converted Ledger Seed to the Vault")?;;
                    id
                }
            };

            WalletAccount {
                blockchain,
                address: kf.address,
                key: PKType::SeedHd(SeedRef {
                    seed_id,
                    hd_path: data.hd_path.clone()
                })
            }
        }
    };

    let wallet = Wallet {
        id: Uuid::new_v4(),
        label: extract_label(kf),
        accounts: vec![account]
    };
    let wallet_id = wallet.get_id();
    vault.wallets().add(wallet);
    Ok(wallet_id)
}