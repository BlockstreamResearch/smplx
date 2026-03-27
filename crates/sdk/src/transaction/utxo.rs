use simplicityhl::elements::{OutPoint, TxOut, TxOutSecrets};

pub struct UTXO {
    pub outpoint: OutPoint,
    pub txout: TxOut,
}

pub struct CTXO {
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub secrets: TxOutSecrets,
}
