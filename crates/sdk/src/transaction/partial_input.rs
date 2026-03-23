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

#[derive(Debug, Clone)]
pub struct PartialInput {
    pub witness_txid: Txid,
    pub witness_output_index: u32,
    pub witness_utxo: TxOut,
    pub sequence: Sequence,
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
        Self::new_sequence(outpoint, txout, Default::default())
    }

    pub fn new_sequence(outpoint: OutPoint, txout: TxOut, sequence: Sequence) -> Self {
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
            sequence,
            amount,
            asset,
        }
    }

    pub fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.witness_txid,
            vout: self.witness_output_index,
        }
    }

    pub fn input(&self) -> Input {
        Input {
            previous_txid: self.witness_txid,
            previous_output_index: self.witness_output_index,
            witness_utxo: Some(self.witness_utxo.clone()),
            sequence: Some(self.sequence),
            amount: self.amount,
            asset: self.asset,
            ..Default::default()
        }
    }
}

impl ProgramInput {
    pub fn new(program: Box<dyn ProgramTrait>, witness: Box<dyn WitnessTrait>) -> Self {
        Self { program, witness }
    }
}

impl IssuanceInput {
    pub fn new(issuance_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self {
            issuance_amount,
            asset_entropy,
        }
    }

    pub fn input(&self) -> Input {
        Input {
            issuance_value_amount: Some(self.issuance_amount),
            issuance_asset_entropy: Some(self.asset_entropy),
            issuance_inflation_keys: None,
            blinded_issuance: Some(0x00),
            ..Default::default()
        }
    }
}
