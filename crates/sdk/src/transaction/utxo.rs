use simplicityhl::elements::{OutPoint, TxOut, TxOutSecrets};

#[derive(Debug, Clone)]
pub struct UTXO {
    pub outpoint: OutPoint,
    pub txout: TxOut,
    pub secrets: Option<TxOutSecrets>,
}
