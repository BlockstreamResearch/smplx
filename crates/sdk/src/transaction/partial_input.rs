use std::fmt;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use elements_miniscript::bitcoin::bip32::DerivationPath;

use simplicityhl::WitnessValues;
use simplicityhl::elements::confidential::{Asset, Value};
use simplicityhl::elements::pset::Input;
use simplicityhl::elements::{AssetId, LockTime, OutPoint, Sequence, TxOut, TxOutSecrets, Txid};
use simplicityhl::simplicity::hashes::Hash;

use crate::program::ProgramTrait;
use crate::program::WitnessTrait;
use crate::utils::tagged_hash;

use super::UTXO;

/// Derives the 32-byte message to sign from the input's `sighash_all`.
pub type SigMessageFn = Arc<dyn Fn([u8; 32]) -> [u8; 32] + Send + Sync>;

/// Defines the 32-byte message a witness signature actually covers.
#[derive(Clone)]
pub enum SigMessage {
    /// Sign `sighash_all` itself.
    Sighash,
    /// Sign the BIP-340 tagged hash `sha256(sha256(tag) || sha256(tag) || sighash_all)`.
    Tagged(String),
    /// Sign whatever the closure derives from `sighash_all`.
    Custom(SigMessageFn),
}

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
    /// Like `WitnessWithPath`, but over a message derived from `sighash_all`.
    WitnessWithMessage(String, Vec<String>, SigMessage),
}

impl SigMessage {
    /// Derives the message to sign from the input's `sighash_all`.
    #[must_use]
    pub fn digest(&self, sighash: [u8; 32]) -> [u8; 32] {
        match self {
            Self::Sighash => sighash,
            Self::Tagged(tag) => tagged_hash(tag, &sighash).to_byte_array(),
            Self::Custom(derive) => derive(sighash),
        }
    }
}

impl fmt::Debug for SigMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sighash => f.write_str("Sighash"),
            Self::Tagged(tag) => f.debug_tuple("Tagged").field(tag).finish(),
            Self::Custom(_) => f.write_str("Custom(<closure>)"),
        }
    }
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

    /// Creates a `WitnessWithMessage` requirement using an iterator of path segments.
    pub fn witness_with_message<I>(name: &str, path: I, message: SigMessage) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        RequiredSignature::WitnessWithMessage(
            name.to_string(),
            path.into_iter().map(|s| s.as_ref().to_string()).collect(),
            message,
        )
    }

    /// Creates a requirement for a BIP-340 tagged signature over `tag` and `sighash_all`.
    pub fn witness_tagged<I>(name: &str, path: I, tag: &str) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        Self::witness_with_message(name, path, SigMessage::Tagged(tag.to_string()))
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
        writeln!(f, "{}", self.witness)
    }
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
            derivation_path: None,
        }
    }

    /// Sets the derivation path, relative to the account path, of the key that spends this input.
    ///
    /// Relative means it is appended to `m/84h/{coin}h/0h`.
    /// Should be in the form of "m/n".
    #[must_use]
    pub fn with_derivation_path(mut self, derivation_path: DerivationPath) -> Self {
        self.derivation_path = Some(derivation_path);

        self
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
        let height_locktime = match self.locktime {
            LockTime::Blocks(value) if value.to_consensus_u32() > 0 => Some(value),
            LockTime::Blocks(_) | LockTime::Seconds(_) => None,
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
            issuance_value_amount: Some(issuance_amount),
            issuance_asset_entropy: Some(asset_entropy),
            issuance_inflation_keys: inflation_amount,
            blinded_issuance: Some(0x00),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIGHASH: [u8; 32] = [0x11; 32];

    #[test]
    fn sighash_message_is_the_sighash() {
        assert_eq!(SigMessage::Sighash.digest(SIGHASH), SIGHASH);
    }

    #[test]
    fn tagged_message_is_the_tagged_hash() {
        assert_eq!(
            SigMessage::Tagged("SimplexTag".to_string()).digest(SIGHASH),
            tagged_hash("SimplexTag", &SIGHASH).to_byte_array()
        );
    }

    #[test]
    fn custom_message_receives_the_sighash() {
        let message = SigMessage::Custom(Arc::new(|sighash: [u8; 32]| {
            let mut digest = sighash;
            digest[0] ^= 0xff;

            digest
        }));

        let mut expected = SIGHASH;
        expected[0] ^= 0xff;

        assert_eq!(message.digest(SIGHASH), expected);
    }

    #[test]
    fn tagged_constructor_builds_a_tagged_message() {
        let required = RequiredSignature::witness_tagged("SIGNATURE", ["Left", "1"], "SimplexTag");

        let RequiredSignature::WitnessWithMessage(name, path, message) = required else {
            panic!("expected a WitnessWithMessage requirement");
        };

        assert_eq!(name, "SIGNATURE");
        assert_eq!(path, ["Left", "1"]);
        assert_eq!(
            message.digest(SIGHASH),
            tagged_hash("SimplexTag", &SIGHASH).to_byte_array()
        );
    }
}
