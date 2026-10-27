use std::fmt::{Debug, Formatter};

use elements_miniscript::bitcoin::bip32::DerivationPath;

use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetEntropy, AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets};

use crate::program::ProgramTrait;
use crate::utils;

use super::UTXO;

/// The output an input spends, with the input's own sequence, locktime and signing key.
///
/// Create one from a [`UTXO`] with [`PartialInput::new`].
#[derive(Debug, Clone)]
pub struct PartialInput {
    /// The outpoint of the UTXO being spent.
    pub outpoint: OutPoint,
    /// The output being spent.
    pub witness_utxo: TxOut,
    /// This input's sequence. `None` takes the transaction's,
    /// [`FinalTransaction::sequence`](super::FinalTransaction::sequence).
    pub sequence: Option<Sequence>,
    /// This input's locktime. `None` takes the transaction's,
    /// [`FinalTransaction::locktime`](super::FinalTransaction::locktime).
    pub locktime: Option<LockTime>,
    /// The unblinded asset, amount and blinding factors of a confidential output.
    ///
    /// A confidential input needs these, because the signer must know every input's asset and
    /// amount. `Signer::get_utxos` and its siblings return wallet UTXOs with them already set.
    pub secrets: Option<TxOutSecrets>,
    /// The path of the key that signs this input, relative to the signer's account path
    /// `m/84h/{coin}h/0h`. `None` means the default path, `0/0`.
    ///
    /// A wallet whose UTXOs sit at different derivation indices must set this on each input, or
    /// the signer signs with the wrong key.
    pub derivation_path: Option<DerivationPath>,
}

/// The Simplicity program that locks an input's spent output, and the witness that satisfies it.
#[derive(Clone)]
pub struct ProgramInput {
    /// The program. Its address must be the spent output's script.
    pub program: Box<dyn ProgramTrait>,
    /// The witness. A field that a [`ProgramSignature`](super::ProgramSignature) names holds a
    /// placeholder, which the signer replaces with the signature.
    pub witness: WitnessValues,
}

impl Debug for ProgramInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProgramInput")
            .field("witness", &self.witness)
            .finish_non_exhaustive()
    }
}

/// The assets an issuance creates, or a reissuance mints again.
///
/// Returned by [`FinalTransaction::add_issuance_input`](super::FinalTransaction::add_issuance_input)
/// and [`FinalTransaction::add_program_issuance_input`](super::FinalTransaction::add_program_issuance_input).
#[derive(Debug, Clone)]
pub struct IssuanceDetails {
    /// The issued asset.
    pub asset_id: AssetId,
    /// The reissuance token, which a later reissuance of the asset must spend.
    pub inflation_asset_id: AssetId,
    /// The asset entropy both IDs derive from. Pass its bytes to
    /// [`IssuanceInput::new_reissuance`] to reissue the asset.
    pub asset_entropy: AssetEntropy,
}

/// An asset issuance or reissuance carried by an input.
///
/// Issued amounts are always explicit.
#[derive(Clone, Debug)]
pub enum IssuanceInput {
    /// Issues a new asset, whose ID derives from the input's outpoint and `asset_entropy`.
    Issuance {
        /// The amount of the asset to issue.
        issuance_amount: u64,
        /// The amount of reissuance tokens to issue. With 0, the asset can't be reissued.
        inflation_amount: u64,
        /// The contract hash.
        asset_entropy: [u8; 32],
    },
    /// Mints more of an existing asset. The input must spend a confidential output that holds the
    /// asset's reissuance token.
    Reissuance {
        /// The amount of the asset to mint.
        issuance_amount: u64,
        /// The asset entropy, from [`IssuanceDetails::asset_entropy`].
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
    /// Creates an input that spends `utxo`, keeping its secrets, with no sequence, locktime or
    /// derivation path of its own.
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

    /// Sets the path of the key that signs this input, relative to the signer's account path
    /// `m/84h/{coin}h/0h`.
    ///
    /// The path must have exactly two unhardened steps, such as `0/5`. The signer returns
    /// `SignerError::InvalidDerivationPath` for any other path when it signs the input.
    #[must_use]
    pub fn with_derivation_path(mut self, derivation_path: DerivationPath) -> Self {
        self.derivation_path = Some(derivation_path);

        self
    }

    /// Sets this input's sequence, overriding the transaction's.
    #[must_use]
    pub fn with_sequence(mut self, sequence: Sequence) -> Self {
        self.sequence = Some(sequence);

        self
    }

    /// Sets this input's locktime, overriding the transaction's.
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

    /// Converts this input into a PSET input.
    ///
    /// An unset sequence becomes `Sequence::MAX` and an unset locktime becomes zero. The
    /// transaction-wide values aren't applied here;
    /// [`FinalTransaction::extract_pst`](super::FinalTransaction::extract_pst) applies them.
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
    /// Pairs a program with its witness.
    #[must_use]
    pub fn new(program: Box<dyn ProgramTrait>, witness: impl Into<WitnessValues>) -> Self {
        Self {
            program,
            witness: witness.into(),
        }
    }
}

impl IssuanceInput {
    /// Issues `issuance_amount` of a new asset and `inflation_amount` of its reissuance token.
    ///
    /// `asset_entropy` is the contract hash. The asset's entropy derives from it and the
    /// outpoint of the input that carries the issuance.
    #[must_use]
    pub fn new_issuance(issuance_amount: u64, inflation_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Issuance {
            issuance_amount,
            inflation_amount,
            asset_entropy,
        }
    }

    /// Mints `issuance_amount` more of the asset whose entropy is `asset_entropy`.
    ///
    /// `asset_entropy` is the already derived entropy, such as
    /// `details.asset_entropy.to_byte_array()` for the [`IssuanceDetails`] of the first issuance.
    #[must_use]
    pub fn new_reissuance(issuance_amount: u64, asset_entropy: [u8; 32]) -> Self {
        Self::Reissuance {
            issuance_amount,
            asset_entropy,
        }
    }

    /// The issuance fields of a PSET input. The other fields are left at their defaults.
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
