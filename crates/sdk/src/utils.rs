use bitcoin_hashes::HashEngine;
use sha2::{Digest, Sha256};

use bip39::Mnemonic;

use simplicityhl::elements::{AssetId, ContractHash, OutPoint, Script};
use simplicityhl::simplicity::bitcoin;
use simplicityhl::simplicity::bitcoin::secp256k1;
use simplicityhl::simplicity::hashes::{Hash, sha256};

/// Generates a radom menemonic with 12 words.
///
/// # Panics
/// Panics if the underlying mnemonic generation fails (e.g. invalid word count configuration).
#[must_use]
pub fn random_mnemonic() -> String {
    let mnemonic = Mnemonic::generate(12).expect("word count should be valid");

    mnemonic.to_string()
}

/// Generates a hardcoded "unspendable" Taproot public key.
///
/// # Panics
/// Panics if the hardcoded byte slice is not exactly 32 bytes or cannot be parsed into a valid `XOnlyPublicKey` (which should never happen statically).
#[must_use]
pub fn tr_unspendable_key() -> secp256k1::XOnlyPublicKey {
    secp256k1::XOnlyPublicKey::from_slice(&[
        0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e, 0x07, 0x8a,
        0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
    ])
    .expect("key should be valid")
}

/// Calculates new sha256 midstate and binds generated entropy to the specific outpoint.
#[must_use]
pub fn asset_entropy(outpoint: &OutPoint, entropy: [u8; 32]) -> sha256::Midstate {
    let contract_hash = ContractHash::from_byte_array(entropy);
    AssetId::generate_asset_entropy(*outpoint, contract_hash)
}

/// Hashes arbitrary data with and additional `TapData` and tags.
#[must_use]
pub fn tap_data_hash(data: &[u8]) -> sha256::Hash {
    let tag = sha256::Hash::hash(b"TapData");

    let mut eng = sha256::Hash::engine();
    eng.input(tag.as_byte_array());
    eng.input(tag.as_byte_array());
    eng.input(data);

    sha256::Hash::from_engine(eng)
}

/// Computes the SHA-256 hash of a given script and returns the resulting 32-byte array.
#[must_use]
pub fn hash_script(script: &Script) -> [u8; 32] {
    let mut hasher = Sha256::new();

    sha2::digest::Update::update(&mut hasher, script.as_bytes());
    hasher.finalize().into()
}

/// Converts Satoshi amount into BTC.
#[must_use]
pub fn sat2btc(sat: u64) -> f64 {
    bitcoin::Amount::from_sat(sat).to_btc()
}

/// Converts BTC amount into Satoshi.
#[must_use]
pub fn btc2sat(btc: u64) -> u64 {
    bitcoin::Amount::from_int_btc(btc).to_sat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_mnemonic() {
        let _ = random_mnemonic();
    }
}
