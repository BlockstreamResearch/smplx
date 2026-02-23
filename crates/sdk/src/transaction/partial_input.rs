use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, OutPoint, Sequence, TxOut, Txid};

#[derive(Clone)]
pub struct PartialInput {
    pub witness_txid: Txid,
    pub witness_output_index: u32,
    pub witness_utxo: TxOut,
    pub sequence: Option<Sequence>,
    pub amount: Option<u64>,
    pub asset: Option<AssetId>,
}

impl PartialInput {
    pub fn new(outpoint: OutPoint, txout: TxOut) -> Self {
        Self {
            witness_txid: outpoint.txid,
            witness_output_index: outpoint.vout,
            witness_utxo: txout,
            sequence: Default::default(),
            amount: Default::default(),
            asset: Default::default(),
        }
    }

    pub fn to_input(&self) -> Input {
        let mut input = Input::default();

        input.previous_txid = self.witness_txid.clone();
        input.previous_output_index = self.witness_output_index;
        input.witness_utxo = Some(self.witness_utxo.clone());
        input.sequence = self.sequence.clone();
        input.amount = self.amount.clone();
        input.asset = self.asset.clone();

        input
    }
}
