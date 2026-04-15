use simplicityhl::elements::confidential::{Asset, Value};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets, Txid};

use crate::program::ProgramTrait;
use crate::program::WitnessTrait;

use super::UTXO;

#[derive(Debug, Clone)]
pub enum RequiredSignature {
    None,
    NativeEcdsa,
    Witness(String),
    WitnessWithPath(String, Vec<String>),
}

#[derive(Debug, Clone)]
pub struct PartialInput {
    pub witness_txid: Txid,
    pub witness_output_index: u32,
    pub witness_utxo: TxOut,
    pub sequence: Sequence,
    pub locktime: LockTime,
    // if utxo is explicit, amount and asset are Some
    pub amount: Option<u64>,
    pub asset: Option<AssetId>,
    // if utxo is confidential, secrets are Some
    pub secrets: Option<TxOutSecrets>,
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
    pub fn new(utxo: UTXO) -> Self {
        let amount = match utxo.txout.value {
            Value::Explicit(value) => Some(value),
            _ => None,
        };
        let asset = match utxo.txout.asset {
            Asset::Explicit(asset) => Some(asset),
            _ => None,
        };

        Self {
            witness_txid: utxo.outpoint.txid,
            witness_output_index: utxo.outpoint.vout,
            witness_utxo: utxo.txout,
            sequence: Sequence::default(),
            locktime: LockTime::ZERO,
            amount,
            asset,
            secrets: utxo.secrets,
        }
    }

    pub fn with_sequence(mut self, sequence: Sequence) -> Self {
        self.sequence = sequence;

        self
    }

    pub fn with_locktime(mut self, locktime: LockTime) -> Self {
        self.locktime = locktime;

        self
    }

    pub fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.witness_txid,
            vout: self.witness_output_index,
        }
    }

    pub fn to_input(&self) -> Input {
        let time_locktime = match self.locktime {
            LockTime::Seconds(value) => Some(value),
            _ => None,
        };
        // zero height locktime is essentially ignored
        let height_locktime = match self.locktime {
            LockTime::Blocks(value) => Some(value),
            _ => None,
        };

        Input {
            previous_txid: self.witness_txid,
            previous_output_index: self.witness_output_index,
            witness_utxo: Some(self.witness_utxo.clone()),
            sequence: Some(self.sequence),
            required_time_locktime: time_locktime,
            required_height_locktime: height_locktime,
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

    pub fn to_input(&self) -> Input {
        Input {
            issuance_value_amount: Some(self.issuance_amount),
            issuance_asset_entropy: Some(self.asset_entropy),
            issuance_inflation_keys: None,
            blinded_issuance: Some(0x00),
            ..Default::default()
        }
    }
}
