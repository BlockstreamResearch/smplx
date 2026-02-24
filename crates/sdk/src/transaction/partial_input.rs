use simplicityhl::elements::confidential::{Asset, Value};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, OutPoint, Sequence, TxOut, Txid};

use crate::program::program::ProgramTrait;
use crate::program::witness::WitnessTrait;

#[derive(Clone)]
pub enum RequiredSignature {
    None,
    NativeEcdsa,
    Witness(String),
}

#[derive(Clone)]
pub struct PartialInput {
    pub witness_txid: Txid,
    pub witness_output_index: u32,
    pub witness_utxo: TxOut,
    pub sequence: Option<Sequence>,
    pub amount: Option<u64>,
    pub asset: Option<AssetId>,
}

#[derive(Clone)]
pub struct ProgramInput {
    pub program: Box<dyn ProgramTrait>,
    pub witness: Box<dyn WitnessTrait>,
}

impl PartialInput {
    pub fn new(outpoint: OutPoint, txout: TxOut) -> Self {
        let amount = match txout.value {
            Value::Explicit(value) => Some(value),
            _ => None,
        };
        let asset = match txout.asset {
            Asset::Explicit(asset) => Some(asset),
            _ => None,
        };

        Self {
            witness_txid: outpoint.txid,
            witness_output_index: outpoint.vout,
            witness_utxo: txout,
            sequence: Default::default(),
            amount: amount,
            asset: asset,
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

impl ProgramInput {
    pub fn new(program: Box<dyn ProgramTrait>, witness: Box<dyn WitnessTrait>) -> Self {
        Self {
            program: program,
            witness: witness,
        }
    }
}
