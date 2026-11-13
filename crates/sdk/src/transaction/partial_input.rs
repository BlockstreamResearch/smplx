use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use elements_miniscript::bitcoin::bip32::DerivationPath;

use simplicityhl::elements::confidential::{self, AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::secp256k1_zkp::SECP256K1;
use simplicityhl::elements::{AssetEntropy, AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets};
use simplicityhl::value::ValueConstructible;
use simplicityhl::{TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use crate::program::ProgramTrait;
use crate::utils;

use super::UTXO;
use super::error::{PartialInputError, ProgramInputError, WitnessWrappingError};
use super::required_signature::ProgramSignature;
use super::witness_injector::WitnessInjector;

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

/// The Simplicity program that locks an input's spent output, and the witness that satisfies the
/// program.
#[derive(Clone)]
pub struct ProgramInput {
    /// The program. Its address must be the spent output's script.
    pub program: Box<dyn ProgramTrait>,
    /// The witness. A field that a [`ProgramSignature`] names holds a
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

    /// Checks that [`PartialInput::asset_and_amount`] can be trusted: the asset and amount of the
    /// spent UTXO are known, and the unblinding secrets, if any, open the UTXO. Opening means the
    /// secrets rebuild its asset and amount, whether blinded or explicit. An explicit asset or
    /// amount needs a zero blinding factor.
    ///
    /// # Errors
    /// Returns `PartialInputError::UnknownAmount` if the UTXO is confidential and has no secrets,
    /// `PartialInputError::AssetMismatch` if the secrets don't open the asset, and
    /// `PartialInputError::AmountMismatch` if they open the asset but not the amount.
    pub fn check_asset_and_amount(&self) -> Result<(), PartialInputError> {
        let Some(secrets) = self.secrets else {
            return self
                .asset_and_amount()
                .map(|_| ())
                .ok_or(PartialInputError::UnknownAmount(self.outpoint));
        };

        let opens_asset = match self.witness_utxo.asset {
            confidential::Asset::Explicit(asset) => {
                asset == secrets.asset && secrets.asset_bf == AssetBlindingFactor::zero()
            }
            committed => committed == confidential::Asset::new_confidential(SECP256K1, secrets.asset, secrets.asset_bf),
        };

        if !opens_asset {
            return Err(PartialInputError::AssetMismatch(self.outpoint));
        }

        let opens_amount = match self.witness_utxo.value {
            confidential::Value::Explicit(value) => {
                value == secrets.value && secrets.value_bf == ValueBlindingFactor::zero()
            }
            committed => {
                committed
                    == confidential::Value::new_confidential_from_assetid(
                        SECP256K1,
                        secrets.value,
                        secrets.asset,
                        secrets.value_bf,
                        secrets.asset_bf,
                    )
            }
        };

        if !opens_amount {
            return Err(PartialInputError::AmountMismatch(self.outpoint));
        }

        Ok(())
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

    /// The witness with `signature` put where `program_signature` says.
    ///
    /// # Errors
    /// For a signature with a path, returns `ProgramInputError::Program` if the program doesn't
    /// compile, and `ProgramInputError::Injection` if the program has no field the signature names
    /// or the path doesn't fit the field.
    ///
    /// # Panics
    /// Panics if the program has the field but the witness doesn't contain the field.
    pub(crate) fn signed_witness(
        &self,
        program_signature: &ProgramSignature,
        signature: [u8; 64],
    ) -> Result<WitnessValues, ProgramInputError> {
        let witness_name = TemplateProgramWitness::witness_from_str(program_signature.name());
        let sig_path = program_signature.path();

        // Inject the signature into the witness field directly if the path is not provided
        let sig_val = if sig_path.is_empty() {
            Value::byte_array(signature)
        } else {
            let witness_types = self.program.get_witness_types()?;
            let witness_type = witness_types
                .get(&witness_name)
                .ok_or(WitnessWrappingError::WitnessFieldNotFound(
                    program_signature.name().to_string(),
                ))?;
            let local_witness = Arc::new(self.witness.get(&witness_name).expect("checked above").clone());

            WitnessInjector::inject_value(&local_witness, witness_type, sig_path, Value::byte_array(signature))?
        };

        let mut hm: HashMap<_, _> = self
            .witness
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();

        hm.insert(witness_name, sig_val);

        Ok(WitnessValues::from_map(hm))
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

#[cfg(test)]
mod tests {
    use bip39::rand::thread_rng;

    use super::*;

    #[test]
    fn secrets_must_open_the_spent_utxo() {
        let asset = AssetId::from_byte_array([0x01; 32]);
        let other_asset = AssetId::from_byte_array([0x02; 32]);
        let asset_bf = AssetBlindingFactor::new(&mut thread_rng());
        let value_bf = ValueBlindingFactor::new(&mut thread_rng());

        let blinded = TxOut {
            asset: confidential::Asset::new_confidential(SECP256K1, asset, asset_bf),
            value: confidential::Value::new_confidential_from_assetid(SECP256K1, 1000, asset, value_bf, asset_bf),
            ..TxOut::default()
        };
        let explicit = TxOut::new_fee(1000, asset);
        let check = |txout: &TxOut, secrets: TxOutSecrets| {
            PartialInput::new(UTXO {
                outpoint: OutPoint::default(),
                txout: txout.clone(),
                secrets: Some(secrets),
            })
            .check_asset_and_amount()
        };

        assert_eq!(
            check(&blinded, TxOutSecrets::new(asset, asset_bf, 1000, value_bf)),
            Ok(())
        );
        assert_eq!(
            check(&blinded, TxOutSecrets::new(other_asset, asset_bf, 1000, value_bf)),
            Err(PartialInputError::AssetMismatch(OutPoint::default()))
        );
        assert_eq!(
            check(
                &blinded,
                TxOutSecrets::new(asset, asset_bf, 1000, ValueBlindingFactor::zero())
            ),
            Err(PartialInputError::AmountMismatch(OutPoint::default()))
        );

        // Secrets of an explicit UTXO state its asset and amount with zero factors
        assert_eq!(
            check(
                &explicit,
                TxOutSecrets::new(asset, AssetBlindingFactor::zero(), 1000, ValueBlindingFactor::zero())
            ),
            Ok(())
        );
        assert_eq!(
            check(
                &explicit,
                TxOutSecrets::new(asset, AssetBlindingFactor::zero(), 999, ValueBlindingFactor::zero())
            ),
            Err(PartialInputError::AmountMismatch(OutPoint::default()))
        );
    }
}
