use simplicityhl::elements::{AssetId, OutPoint, TxOut, TxOutSecrets};

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

impl UTXO {
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

    /// Retrieves the `AssetId` associated with the instance.
    #[must_use]
    pub fn asset(&self) -> AssetId {
        self.secrets
            .map_or_else(|| self.explicit_asset(), |secrets| secrets.asset)
    }

    /// Retrieves the amount associated with the current instance.
    ///
    /// This function returns the `value` from the `secrets` field if it exists.
    /// If no `secrets` are present, it falls back to calculating and returning
    /// the explicitly defined amount using the `explicit_amount()` method.
    #[must_use]
    pub fn amount(&self) -> u64 {
        self.secrets
            .map_or_else(|| self.explicit_amount(), |secrets| secrets.value)
    }
}
