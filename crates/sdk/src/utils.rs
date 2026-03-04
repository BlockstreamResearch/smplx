use simplicityhl::simplicity::bitcoin;
use simplicityhl::simplicity::bitcoin::secp256k1;

use simplicityhl::elements::{AssetId, ContractHash, OutPoint};
use simplicityhl::simplicity::hashes::{Hash, sha256};

pub fn tr_unspendable_key() -> secp256k1::XOnlyPublicKey {
    secp256k1::XOnlyPublicKey::from_slice(&[
        0x50, 0x92, 0x9b, 0x74, 0xc1, 0xa0, 0x49, 0x54, 0xb7, 0x8b, 0x4b, 0x60, 0x35, 0xe9, 0x7a, 0x5e, 0x07, 0x8a,
        0x5a, 0x0f, 0x28, 0xec, 0x96, 0xd5, 0x47, 0xbf, 0xee, 0x9a, 0xce, 0x80, 0x3a, 0xc0,
    ])
    .expect("key should be valid")
}

pub fn asset_entropy(outpoint: &OutPoint, entropy: [u8; 32]) -> sha256::Midstate {
    let contract_hash = ContractHash::from_byte_array(entropy);
    AssetId::generate_asset_entropy(*outpoint, contract_hash)
}

pub fn sat2btc(sat: u64) -> String {
    let amount = bitcoin::Amount::from_sat(sat);
    amount.to_string_in(bitcoin::amount::Denomination::Bitcoin)
}
