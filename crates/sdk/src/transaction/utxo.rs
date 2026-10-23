use simplicityhl::elements::{AssetId, OutPoint, Script, TxOut, TxOutSecrets};

use super::error::UtxoError;

/// Represents an Unspent Transaction Output (UTXO).
#[derive(Debug, Clone)]
pub struct UTXO {
    /// Bounded outpoint for this object
    pub outpoint: OutPoint,
    /// Transaction output characteristics
    pub txout: TxOut,
    /// Already unblinded transaction output secrets
    pub secrets: Option<TxOutSecrets>,
}

impl Default for UTXO {
    fn default() -> Self {
        Self::explicit(OutPoint::default(), Script::new(), 0, AssetId::default())
    }
}

impl UTXO {
    /// Creates a UTXO with an explicit amount and asset, locked by `script`.
    #[must_use]
    pub fn explicit(outpoint: OutPoint, script: Script, amount: u64, asset: AssetId) -> Self {
        let mut txout = TxOut::new_fee(amount, asset);
        txout.script_pubkey = script;

        Self {
            outpoint,
            txout,
            secrets: None,
        }
    }

    /// Retrieves the explicit `AssetId` from the transaction output (`txout`).
    ///
    /// # Panics
    /// This function will panic if the UTXO's asset is confidential.
    #[must_use]
    pub fn explicit_asset(&self) -> AssetId {
        self.txout.asset.explicit().expect("The UTXO's asset is not explicit")
    }

    /// Retrieves the explicit amount contained within the transaction output (UTXO).
    ///
    /// # Panics
    /// This function will panic if the UTXO's amount is confidential.
    #[must_use]
    pub fn explicit_amount(&self) -> u64 {
        self.txout.value.explicit().expect("The UTXO's amount is not explicit")
    }

    /// Retrieves the unblinded `AssetId` of the current UTXO.
    ///
    /// # Panics
    ///
    /// This function will panic if the UTXO is not blinded. The panic occurs when
    /// `self.secrets` is `None`, as it expects the UTXO to be in an unblinded state to retrieve the `AssetId`.
    #[must_use]
    pub fn unblinded_asset(&self) -> AssetId {
        self.secrets.expect("The UTXO is not unblinded").asset
    }

    /// Retrieves the unblinded amount from the UTXO.
    ///
    /// # Panics
    /// This function will panic if the UTXO is not confidential.
    #[must_use]
    pub fn unblinded_amount(&self) -> u64 {
        self.secrets.expect("The UTXO is not unblinded").value
    }

    /// Retrieves the asset ID from unblinding secrets or the explicit output field.
    ///
    /// # Errors
    /// Returns `UtxoError::UnknownAsset` if the asset is confidential or null and
    /// no unblinding secrets are available.
    pub fn asset(&self) -> Result<AssetId, UtxoError> {
        self.secrets
            .map(|secrets| secrets.asset)
            .or_else(|| self.txout.asset.explicit())
            .ok_or(UtxoError::UnknownAsset(self.outpoint))
    }

    /// Retrieves the amount from unblinding secrets or the explicit output field.
    ///
    /// # Errors
    /// Returns `UtxoError::UnknownAmount` if the value is confidential or null and
    /// no unblinding secrets are available.
    pub fn amount(&self) -> Result<u64, UtxoError> {
        self.secrets
            .map(|secrets| secrets.value)
            .or_else(|| self.txout.value.explicit())
            .ok_or(UtxoError::UnknownAmount(self.outpoint))
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::elements::Txid;
    use simplicityhl::elements::confidential::{Asset, Value};
    use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
    use simplicityhl::elements::secp256k1_zkp::Secp256k1;

    use crate::provider::SimplicityNetwork;
    use crate::signer::Signer;

    use super::*;

    #[test]
    fn explicit_and_unblinded_accessors_select_the_right_values() {
        let asset = AssetId::from_byte_array([1; 32]);
        let explicit = UTXO {
            outpoint: OutPoint::default(),
            txout: TxOut {
                asset: Asset::Explicit(asset),
                value: Value::Explicit(42),
                ..Default::default()
            },
            secrets: None,
        };

        assert_eq!(explicit.asset(), Ok(asset));
        assert_eq!(explicit.amount(), Ok(42));

        let unblinded = UTXO {
            outpoint: OutPoint::default(),
            txout: TxOut::default(),
            secrets: Some(TxOutSecrets::new(
                asset,
                AssetBlindingFactor::zero(),
                99,
                ValueBlindingFactor::zero(),
            )),
        };

        assert_eq!(unblinded.unblinded_asset(), asset);
        assert_eq!(unblinded.unblinded_amount(), 99);
        assert_eq!(unblinded.asset(), Ok(asset));
        assert_eq!(unblinded.amount(), Ok(99));
    }

    fn explicit(network: &SimplicityNetwork, amount: u64) -> UTXO {
        const MNEMONIC: &str =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

        let signer = Signer::from_mnemonic(MNEMONIC, *network);

        UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), 0),
            txout: TxOut {
                script_pubkey: signer.get_address().script_pubkey(),
                ..TxOut::new_fee(amount, network.policy_asset())
            },
            secrets: None,
        }
    }

    #[test]
    fn unresolved_utxo_accessors_do_not_panic() {
        let network = SimplicityNetwork::default_regtest();

        let mixed = {
            let asset_bf = AssetBlindingFactor::new(&mut bip39::rand::thread_rng());

            let mut utxo = explicit(&network, 600);
            utxo.txout.asset = Asset::new_confidential(&Secp256k1::new(), network.policy_asset(), asset_bf);

            // An explicit value has no rangeproof to rewind for the asset blinding factor.
            assert!(utxo.txout.witness.rangeproof.is_empty());

            utxo
        };

        let null_asset = {
            let mut utxo = explicit(&network, 600);
            utxo.txout.asset = Asset::Null;

            utxo
        };

        for utxo in [mixed, null_asset] {
            assert_eq!(utxo.asset(), Err(UtxoError::UnknownAsset(utxo.outpoint)));
        }
    }

    #[test]
    fn unresolved_utxo_value_does_not_panic() {
        let network = SimplicityNetwork::default_regtest();

        let mut utxo = explicit(&network, 600);
        utxo.txout.value = Value::Null;

        assert_eq!(utxo.amount(), Err(UtxoError::UnknownAmount(utxo.outpoint)));
    }

    #[test]
    fn known_secrets_resolve_an_asset_only_commitment() {
        let asset = AssetId::from_byte_array(Default::default());
        let asset_bf = AssetBlindingFactor::new(&mut bip39::rand::thread_rng());

        let mut utxo = UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), 0),
            txout: TxOut {
                asset: Asset::new_confidential(&Secp256k1::new(), asset, asset_bf),
                value: Value::Explicit(600),
                ..Default::default()
            },
            secrets: None,
        };

        // The visible amount cannot reveal the committed asset ID.
        assert_eq!(utxo.amount(), Ok(600));
        assert_eq!(utxo.asset(), Err(UtxoError::UnknownAsset(utxo.outpoint)));

        utxo.secrets = Some(TxOutSecrets::new(asset, asset_bf, 600, ValueBlindingFactor::zero()));
        assert_eq!(utxo.asset(), Ok(asset));
        assert_eq!(utxo.amount(), Ok(600));
    }
}
