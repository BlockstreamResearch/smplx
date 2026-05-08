use simplicityhl::elements::confidential::{Asset, Value};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets, Txid};

use crate::program::ProgramTrait;
use crate::program::WitnessTrait;

use super::UTXO;

/// Defines the type of signature required for an input.
#[derive(Debug, Clone)]
pub enum RequiredSignature {
    /// No signature is required.
    None,
    /// A standard Native ECDSA (WPKH) signature is required.
    NativeEcdsa,
    /// A generic witness payload associated with an external name.
    Witness(String),
    /// A witness payload requiring traversal through a specified path hierarchy.
    WitnessWithPath(String, Vec<String>),
}

impl RequiredSignature {
    /// Creates a `WitnessWithPath` requirement using an iterator of path segments.
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

/// Represents partially prepared input data for Elements transactions.
#[derive(Debug, Clone)]
pub struct PartialInput {
    /// The transaction ID containing the target UTXO being spent.
    pub witness_txid: Txid,
    /// The output index of the UTXO within the transaction being spent.
    pub witness_output_index: u32,
    /// The native transaction output corresponding to the targeted UTXO.
    pub witness_utxo: TxOut,
    /// The sequence number indicating transaction replaceability or relative timelocking.
    pub sequence: Sequence,
    /// Absolute timelock criteria enforced against the input.
    pub locktime: LockTime,
    /// The explicit amount value in Satoshis for the input, if available.
    /// Note: if UTXO is explicit, `amount` and `asset` are `Some`.
    pub amount: Option<u64>,
    /// The explicit `AssetId` being spent by the input, if available.
    pub asset: Option<AssetId>,
    /// Optional blinding secrets mapping values and asset states into confidential outputs.
    /// Note: if UTXO is confidential, `secrets` are `Some`.
    pub secrets: Option<TxOutSecrets>,
}

/// Represents an input that runs a specific Simplicity program with an associated witness.
#[derive(Clone)]
pub struct ProgramInput {
    /// The compiled program interface associated with the input.
    pub program: Box<dyn ProgramTrait>,
    /// The witness values required to satisfy the program.
    pub witness: Box<dyn WitnessTrait>,
}

/// Represents an input designated for asset issuance or reissuance.
#[derive(Clone, Debug)]
pub enum IssuanceInput {
    /// Represents a completely new asset issuance.
    Issuance {
        /// The initial issuance amount for the asset.
        issuance_amount: u64,
        /// The initial issuance amount for the inflation key.
        inflation_amount: u64,
        /// The contract hash or entropy used to derive the generated `AssetId`.
        asset_entropy: [u8; 32],
    },
    /// Represents a reissuance of an existing asset.
    Reissuance {
        /// The amount of the generated asset to issue.
        issuance_amount: u64,
        /// The original asset's entropy used to tie this reissuance back to the parent issuance.
        asset_entropy: [u8; 32],
    },
}

impl PartialInput {
    /// Creates a new `PartialInput` from an existing `UTXO`.
    /// Extracts explicit value and asset amounts if available.
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

    /// Sets a specific `Sequence` for the input.
    #[must_use]
    pub fn with_sequence(mut self, sequence: Sequence) -> Self {
        self.sequence = sequence;

        self
    }

    /// Sets a specific `LockTime` for the input.
    #[must_use]
    pub fn with_locktime(mut self, locktime: LockTime) -> Self {
        self.locktime = locktime;

        self
    }

    /// Returns the `OutPoint` corresponding to this input.
    #[must_use]
    pub fn outpoint(&self) -> OutPoint {
        OutPoint {
            txid: self.witness_txid,
            vout: self.witness_output_index,
        }
    }

    /// Converts this `PartialInput` into a fully formed PSET `Input`.
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
    /// Creates a new `ProgramInput` from a `ProgramTrait` and its associated `WitnessTrait`.
    #[must_use]
    pub fn new(program: Box<dyn ProgramTrait>, witness: Box<dyn WitnessTrait>) -> Self {
        Self { program, witness }
    }
}

impl IssuanceInput {
    /// Creates a new `IssuanceInput` for creating a new asset issuance.
    #[must_use]
    pub fn new_issuance(issuance_amount: u64, inflation_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Issuance {
            issuance_amount,
            inflation_amount,
            asset_entropy,
        }
    }

    /// Creates a new `IssuanceInput` for reissuing an existing asset.
    #[must_use]
    pub fn new_reissuance(issuance_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Reissuance {
            issuance_amount,
            asset_entropy,
        }
    }

    /// Converts this `IssuanceInput` into a partial PSET `Input` configured for issuance or reissuance.
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
