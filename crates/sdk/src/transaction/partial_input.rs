use std::fmt::{Debug, Formatter};

use elements_miniscript::bitcoin::bip32::DerivationPath;

use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetEntropy, AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets};

use crate::program::ProgramTrait;
use crate::utils;

use super::UTXO;

/// Represents partially prepared input data for Elements transactions.
#[derive(Debug, Clone)]
pub struct PartialInput {
    /// The outpoint of the UTXO being spent.
    pub outpoint: OutPoint,
    /// The native transaction output corresponding to the targeted UTXO.
    pub witness_utxo: TxOut,
    /// The sequence number indicating transaction replaceability or relative timelocking.
    /// `None` takes the transaction's.
    pub sequence: Option<Sequence>,
    /// Absolute timelock criteria enforced against the input. `None` takes the transaction's.
    pub locktime: Option<LockTime>,
    /// Optional blinding secrets mapping values and asset states into confidential outputs.
    /// Note: if UTXO is confidential, `secrets` are `Some`.
    pub secrets: Option<TxOutSecrets>,
    /// Derivation path of the key that can spend this input, relative to the account path.
    ///
    /// `None` means the signer's default path. A wallet whose UTXOs sit across many
    /// derivation indices must set this per input, or the signer will sign with the wrong key.
    pub derivation_path: Option<DerivationPath>,
}

/// Represents an input that runs a specific Simplicity program with an associated witness.
#[derive(Clone)]
pub struct ProgramInput {
    /// The compiled program interface associated with the input.
    pub program: Box<dyn ProgramTrait>,
    /// The witness values required to satisfy the program.
    pub witness: WitnessValues,
}

impl Debug for ProgramInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramInput")
            .field("witness", &self.witness)
            .finish_non_exhaustive()
    }
}

/// A structure representing the details of token issuance and related metadata.
#[derive(Debug, Clone)]
pub struct IssuanceDetails {
    /// The unique `AssetId` generated from the provided entropy, representing the issued tokens struct.
    pub asset_id: AssetId,
    /// The `AssetId` corresponding to the reissuance (inflation) token, used for minting new tokens.
    pub inflation_asset_id: AssetId,
    /// The entropy value that was used to derive both the `asset_id` and `inflation_asset_id`.
    pub asset_entropy: AssetEntropy,
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

impl Default for PartialInput {
    /// An input spending the default `UTXO`.
    fn default() -> Self {
        Self::new(UTXO::default())
    }
}

impl PartialInput {
    /// Creates a new `PartialInput` from an existing `UTXO`.
    #[must_use]
    pub fn new(utxo: UTXO) -> Self {
        Self {
            outpoint: utxo.outpoint,
            witness_utxo: utxo.txout,
            sequence: None,
            locktime: None,
            secrets: utxo.secrets,
            derivation_path: None,
        }
    }

    /// Sets the derivation path, relative to the account path, of the key that spends this input.
    ///
    /// Relative means it is appended to `m/84h/{coin}h/0h`.
    /// Should be in the form of `m/n`.
    #[must_use]
    pub fn with_derivation_path(mut self, derivation_path: DerivationPath) -> Self {
        self.derivation_path = Some(derivation_path);

        self
    }

    /// Sets a specific `Sequence` for the input.
    #[must_use]
    pub fn with_sequence(mut self, sequence: Sequence) -> Self {
        self.sequence = Some(sequence);

        self
    }

    /// Sets a specific `LockTime` for the input.
    #[must_use]
    pub fn with_locktime(mut self, locktime: LockTime) -> Self {
        self.locktime = Some(locktime);

        self
    }

    /// The asset and amount of the spent UTXO, from its explicit value or its unblinding secrets.
    /// `None` for a confidential UTXO without secrets.
    #[must_use]
    pub fn asset_and_amount(&self) -> Option<(AssetId, u64)> {
        match self.secrets {
            Some(secrets) => Some((secrets.asset, secrets.value)),
            None => self.explicit_asset().zip(self.explicit_amount()),
        }
    }

    /// Converts this `PartialInput` into a fully formed PSET `Input`.
    /// An unset sequence is final and an unset locktime is zero.
    #[must_use]
    pub fn to_input(&self) -> Input {
        let locktime = self.locktime.unwrap_or(LockTime::ZERO);

        let time_locktime = match locktime {
            LockTime::Seconds(value) => Some(value),
            LockTime::Blocks(_) => None,
        };
        let height_locktime = match locktime {
            LockTime::Blocks(value) if value.to_consensus_u32() > 0 => Some(value),
            LockTime::Blocks(_) | LockTime::Seconds(_) => None,
        };

        Input {
            previous_txid: self.outpoint.txid,
            previous_output_index: self.outpoint.vout,
            witness_utxo: Some(self.witness_utxo.clone()),
            sequence: Some(self.sequence.unwrap_or_default()),
            required_time_locktime: time_locktime,
            required_height_locktime: height_locktime,
            amount: self.explicit_amount(),
            asset: self.explicit_asset(),
            ..Default::default()
        }
    }

    /// The amount of the spent UTXO, `None` when it is confidential.
    fn explicit_amount(&self) -> Option<u64> {
        self.witness_utxo.value.explicit()
    }

    /// The asset of the spent UTXO, `None` when it is confidential.
    fn explicit_asset(&self) -> Option<AssetId> {
        self.witness_utxo.asset.explicit()
    }
}

impl ProgramInput {
    /// Creates a new `ProgramInput` from a `ProgramTrait` and its associated `WitnessTrait`.
    #[must_use]
    pub fn new(program: Box<dyn ProgramTrait>, witness: impl Into<WitnessValues>) -> Self {
        Self {
            program,
            witness: witness.into(),
        }
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
            // Elements encodes a zero amount as null, an explicit zero is invalid
            issuance_value_amount: (issuance_amount > 0).then_some(issuance_amount),
            issuance_asset_entropy: Some(AssetEntropy::from_byte_array(asset_entropy)),
            issuance_inflation_keys: inflation_amount,
            blinded_issuance: Some(0x00),
            ..Default::default()
        }
    }

    /// The assets an issuance spending `outpoint` creates, or a reissuance mints again.
    pub(crate) fn details(&self, outpoint: &OutPoint) -> IssuanceDetails {
        let asset_entropy = match self {
            Self::Issuance { asset_entropy, .. } => utils::asset_entropy(outpoint, *asset_entropy),
            Self::Reissuance { asset_entropy, .. } => AssetEntropy::from_byte_array(*asset_entropy),
        };

        IssuanceDetails {
            asset_id: AssetId::from_entropy(asset_entropy),
            inflation_asset_id: AssetId::reissuance_token_from_entropy(asset_entropy, false),
            asset_entropy,
        }
    }
}
