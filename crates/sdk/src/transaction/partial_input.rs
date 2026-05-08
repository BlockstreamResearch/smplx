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

impl RequiredSignature {
    pub fn witness_with_path<I>(name: &str, path: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        RequiredSignature::WitnessWithPath(
            name.to_string(),
            path.into_iter().map(|s| s.as_ref().to_string()).collect(),
        )
    }
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

#[allow(missing_debug_implementations)]
#[derive(Clone)]
pub struct ProgramInput {
    pub program: Box<dyn ProgramTrait>,
    pub witness: Box<dyn WitnessTrait>,
}

#[derive(Clone)]
pub enum IssuanceInput {
    Issuance {
        issuance_amount: u64,
        inflation_amount: u64,
        asset_entropy: [u8; 32],
    },
    Reissuance {
        issuance_amount: u64,
        asset_entropy: [u8; 32],
    },
}

impl PartialInput {
    #[must_use]
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

    #[must_use]
    pub fn with_sequence(mut self, sequence: Sequence) -> Self {
        self.sequence = sequence;

        self
    }

    #[must_use]
    pub fn with_locktime(mut self, locktime: LockTime) -> Self {
        self.locktime = locktime;

        self
    }

    #[must_use]
    pub fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.witness_txid,
            vout: self.witness_output_index,
        }
    }

    #[must_use]
    pub fn to_input(&self) -> Input {
        let time_locktime = match self.locktime {
            LockTime::Seconds(value) => Some(value),
            LockTime::Blocks(_) => None,
        };
        // zero height locktime is essentially ignored
        let height_locktime = match self.locktime {
            LockTime::Blocks(value) => Some(value),
            LockTime::Seconds(_) => None,
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
    #[must_use]
    pub fn new(program: Box<dyn ProgramTrait>, witness: Box<dyn WitnessTrait>) -> Self {
        Self { program, witness }
    }
}

impl IssuanceInput {
    #[must_use]
    pub fn new_issuance(issuance_amount: u64, inflation_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Issuance {
            issuance_amount,
            inflation_amount,
            asset_entropy,
        }
    }

    #[must_use]
    pub fn new_reissuance(issuance_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Reissuance {
            issuance_amount,
            asset_entropy,
        }
    }

    #[must_use]
    pub fn to_input(&self) -> Input {
        let (issuance_amount, asset_entropy, inflation_amount) = match self {
            Self::Issuance {
                issuance_amount,
                inflation_amount,
                asset_entropy,
            } => {
                let inflation_amount = (*inflation_amount > 0).then_some(*inflation_amount);

                (*issuance_amount, *asset_entropy, inflation_amount)
            }
            Self::Reissuance {
                issuance_amount,
                asset_entropy,
            } => (*issuance_amount, *asset_entropy, None),
        };

        Input {
            issuance_value_amount: Some(issuance_amount),
            issuance_asset_entropy: Some(asset_entropy),
            issuance_inflation_keys: inflation_amount,
            blinded_issuance: Some(0x00),
            ..Default::default()
        }
    }
}
