use crate::PUBLIC_SECRET_BLINDER_KEY;

use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::secp256k1_zkp::SecretKey;

/// Derives a deterministic blinder keypair from the hardcoded public secret.
///
/// # Panics
///
/// Panics if the secret key bytes are invalid (should never happen with valid constant).
#[must_use]
pub fn derive_public_blinder_key() -> secp256k1::Keypair {
    secp256k1::Keypair::from_secret_key(
        secp256k1::SECP256K1,
        &SecretKey::from_slice(&PUBLIC_SECRET_BLINDER_KEY).unwrap(),
    )
}
