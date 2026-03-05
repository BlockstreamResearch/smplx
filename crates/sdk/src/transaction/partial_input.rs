use simplicityhl::elements::confidential::{Asset, Value};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, OutPoint, Sequence, TxOut, Txid};

use crate::program::ProgramTrait;
use crate::program::WitnessTrait;

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

#[derive(Clone)]
pub struct IssuanceInput {
    pub issuance_amount: u64,
    pub asset_entropy: [u8; 32],
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

    pub fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.witness_txid.clone(),
            vout: self.witness_output_index,
        }
    }

    pub fn input(&self) -> Input {
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

impl IssuanceInput {
    pub fn new(issuance_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self {
            issuance_amount: issuance_amount,
            asset_entropy: asset_entropy,
        }
    }

    pub fn input(&self) -> Input {
        let mut input = Input::default();

        input.issuance_value_amount = Some(self.issuance_amount);
        input.issuance_asset_entropy = Some(self.asset_entropy);
        input.issuance_inflation_keys = None;
        input.blinded_issuance = Some(0x00);

        input
    }
}
