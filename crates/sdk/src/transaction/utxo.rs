use simplicityhl::elements::{AssetId, OutPoint, TxOut, TxOutSecrets};

#[derive(Debug, Clone)]
pub struct UTXO {
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub secrets: Option<TxOutSecrets>,
}

impl UTXO {
    pub fn explicit_asset(&self) -> AssetId {
        self.txout.asset.explicit().expect("The UTXO's asset is not explicit")
    }

    pub fn explicit_amount(&self) -> u64 {
        self.txout.value.explicit().expect("The UTXO's amount is not explicit")
    }

    pub fn unblinded_asset(&self) -> AssetId {
        self.secrets.expect("The UTXO is not unblinded").asset
    }

    pub fn unblinded_amount(&self) -> u64 {
        self.secrets.expect("The UTXO is not unblinded").value
    }

    pub fn asset(&self) -> AssetId {
        self.secrets
            .map_or_else(|| self.explicit_asset(), |secrets| secrets.asset)
    }

    pub fn amount(&self) -> u64 {
        self.secrets
            .map_or_else(|| self.explicit_amount(), |secrets| secrets.value)
    }
}
