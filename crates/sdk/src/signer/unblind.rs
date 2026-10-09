use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::secp256k1_zkp::{Generator, PedersenCommitment, SECP256K1, SecretKey};
use simplicityhl::elements::{AssetId, TxOut, TxOutSecrets};

use super::error::SignerError;

/// Reads the asset, amount and blinding factors of an output that `blinding_key` opens.
///
/// Three shapes are understood: a fully confidential output, an output that publishes its asset
/// and hides only its amount, and an explicit output, whose blinding factors are zero.
///
/// The result is private wallet data. It must not be handed to anyone the wallet does not trust
/// with its balances.
///
/// # Errors
/// Returns a `SignerError` if the key does not open the output, or if the output is malformed.
pub fn unblind_txout(txout: &TxOut, blinding_key: SecretKey) -> Result<TxOutSecrets, SignerError> {
    match (txout.asset.explicit(), txout.value.explicit(), txout.value.commitment()) {
        (Some(asset), Some(value), _) => Ok(TxOutSecrets::new(
            asset,
            AssetBlindingFactor::zero(),
            value,
            ValueBlindingFactor::zero(),
        )),
        (Some(asset), None, Some(commitment)) => rewind_amount(txout, asset, commitment, blinding_key),
        _ => Ok(txout.unblind(SECP256K1, blinding_key)?),
    }
}

/// Opens the range proof of an output whose asset is explicit and whose amount is not.
///
/// Elements' own unblinding expects both halves to be committed, so this path rewinds the proof
/// against the unblinded generator of the published asset.
fn rewind_amount(
    txout: &TxOut,
    asset: AssetId,
    commitment: PedersenCommitment,
    blinding_key: SecretKey,
) -> Result<TxOutSecrets, SignerError> {
    let proof = txout
        .witness
        .rangeproof
        .as_ref()
        .ok_or(SignerError::AmountRewind("the output carries no range proof"))?;

    let mut shared = txout
        .nonce
        .shared_secret(&blinding_key)
        .ok_or(SignerError::AmountRewind("the output carries no nonce"))?;

    let rewound = proof.rewind(
        SECP256K1,
        commitment,
        shared,
        txout.script_pubkey.as_bytes(),
        Generator::new_unblinded(SECP256K1, asset.into_tag()),
    );

    shared.non_secure_erase();

    let (opening, _) = rewound.map_err(|_| SignerError::AmountRewind("this key does not open the amount"))?;

    // The proof message carries the asset and its blinding factor. For an explicit asset these
    // are the published asset and zero, and anything else is not this output's opening.
    let message_asset = opening
        .message
        .get(..32)
        .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok());

    if message_asset.map(AssetId::from_byte_array) != Some(asset)
        || opening
            .message
            .get(32..64)
            .is_none_or(|bf| bf.iter().any(|byte| *byte != 0))
    {
        return Err(SignerError::AmountRewind(
            "the range proof message does not name the published asset",
        ));
    }

    let value_bf = ValueBlindingFactor::from_slice(opening.blinding_factor.as_ref())
        .map_err(|_| SignerError::AmountRewind("the range proof holds an invalid blinding factor"))?;

    Ok(TxOutSecrets::new(
        asset,
        AssetBlindingFactor::zero(),
        opening.value,
        value_bf,
    ))
}

#[cfg(test)]
mod tests {
    use simplicityhl::elements::confidential::{Asset, Nonce, Value};
    use simplicityhl::elements::secp256k1_zkp::{PublicKey, rand::thread_rng};
    use simplicityhl::elements::{RangeProof, RangeProofMessage, Script};

    use super::*;

    fn asset() -> AssetId {
        AssetId::from_byte_array([0x42; 32])
    }

    /// An output publishing `asset` and hiding `value` behind a range proof to `recipient`.
    fn amount_only(value: u64, recipient: PublicKey, message_asset: AssetId) -> (TxOut, ValueBlindingFactor) {
        let mut rng = thread_rng();
        let script = Script::from(vec![0x51]);
        let value_bf = ValueBlindingFactor::new(&mut rng);
        let generator = Generator::new_unblinded(SECP256K1, asset().into_tag());
        let commitment = PedersenCommitment::new(SECP256K1, value, value_bf.into_inner(), generator);
        let (nonce, shared) = Nonce::with_ephemeral_sk(SECP256K1, SecretKey::new(&mut rng), &recipient);
        let message = RangeProofMessage::new(message_asset, AssetBlindingFactor::zero());
        let proof = RangeProof::new(
            SECP256K1,
            1,
            commitment,
            value,
            value_bf.into_inner(),
            &message.to_byte_array(),
            script.as_bytes(),
            shared,
            0,
            52,
            generator,
        )
        .expect("a range proof");

        let mut txout = TxOut {
            asset: Asset::Explicit(asset()),
            value: Value::Confidential(commitment),
            nonce,
            script_pubkey: script,
            ..TxOut::default()
        };
        txout.witness.rangeproof = proof;

        (txout, value_bf)
    }

    #[test]
    fn explicit_output_opens_with_zero_blinding_factors() {
        let opened = unblind_txout(
            &TxOut::new_fee(5_000, asset()),
            SecretKey::from_slice(&[7; 32]).unwrap(),
        )
        .expect("an explicit output");

        assert_eq!(opened.value, 5_000);
        assert_eq!(opened.asset, asset());
        assert_eq!(opened.asset_bf, AssetBlindingFactor::zero());
        assert_eq!(opened.value_bf, ValueBlindingFactor::zero());
    }

    #[test]
    fn amount_only_output_opens_with_the_recipient_key() {
        let key = SecretKey::from_slice(&[7; 32]).unwrap();
        let (txout, value_bf) = amount_only(12_345, PublicKey::from_secret_key(SECP256K1, &key), asset());

        let opened = unblind_txout(&txout, key).expect("the recipient opens it");

        assert_eq!(opened.asset, asset());
        assert_eq!(opened.value, 12_345);
        assert_eq!(opened.value_bf, value_bf);
        assert_eq!(opened.asset_bf, AssetBlindingFactor::zero());
    }

    #[test]
    fn amount_only_output_refuses_another_key_and_a_mislabelled_proof() {
        let key = SecretKey::from_slice(&[7; 32]).unwrap();
        let recipient = PublicKey::from_secret_key(SECP256K1, &key);

        let (txout, _) = amount_only(1, recipient, asset());
        assert!(matches!(
            unblind_txout(&txout, SecretKey::from_slice(&[8; 32]).unwrap()),
            Err(SignerError::AmountRewind(_))
        ));

        let (mislabelled, _) = amount_only(1, recipient, AssetId::from_byte_array([0x43; 32]));
        assert!(matches!(
            unblind_txout(&mislabelled, key),
            Err(SignerError::AmountRewind(_))
        ));

        let (mut stripped, _) = amount_only(1, recipient, asset());
        stripped.witness.rangeproof = RangeProof::EMPTY;
        assert!(matches!(
            unblind_txout(&stripped, key),
            Err(SignerError::AmountRewind(_))
        ));
    }
}
