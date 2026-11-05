use std::fmt;
use std::sync::Arc;

use crate::utils::tagged_hash;

/// A function that derives the 32-byte message to sign from the input's `sighash_all`, for
/// [`SigMessage::Custom`].
pub type SigMessageFn = Arc<dyn Fn([u8; 32]) -> [u8; 32] + Send + Sync>;

/// The 32-byte message a program signature covers, derived from the input's Simplicity
/// `sighash_all`.
///
/// Choose the message the contract checks the signature against: `Sighash` for a contract that
/// calls `jet::bip_0340_verify` on `jet::sig_all_hash()`, and `Tagged` or `Custom` for one that
/// hashes it first.
#[derive(Clone)]
pub enum SigMessage {
    /// Sign `sighash_all` itself.
    Sighash,
    /// Sign the BIP-340 tagged hash `sha256(sha256(tag) || sha256(tag) || sighash_all)`.
    Tagged(String),
    /// Sign whatever the closure derives from `sighash_all`.
    Custom(SigMessageFn),
}

/// The signature the signer puts on a native input, one not locked by a Simplicity program.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSignature {
    /// An ECDSA signature for a P2WPKH output, with the key at the input's derivation path.
    Ecdsa,
}

/// A Schnorr signature the signer puts into a field of a program input's witness.
///
/// ```ignore
/// // The signature goes into `witness::SIGNATURE` itself.
/// ProgramSignature::new("SIGNATURE");
/// // It goes inside `witness::PATH`, which has the type `Either<_, Either<Signature, _>>`.
/// ProgramSignature::new("PATH").at_path(["Right", "Left"]);
/// ```
///
/// The signer signs the message [`SigMessage`] derives from the input's `sighash_all`, using the
/// key at the input's derivation path.
#[derive(Debug, Clone)]
pub struct ProgramSignature {
    name: String,
    path: Vec<String>,
    message: SigMessage,
}

/// The signature the signer adds to an input.
///
/// [`FinalTransaction`](super::FinalTransaction)'s `add_*` methods take an
/// `Option<NativeSignature>` or an `Option<ProgramSignature>` and convert it into this type.
#[derive(Debug, Clone)]
pub enum RequiredSignature {
    /// The signer doesn't sign the input. A native input stays unsigned, for its owner to sign,
    /// and a program input's witness is used as supplied.
    None,
    /// The signer signs a native input with ECDSA, as [`NativeSignature::Ecdsa`].
    NativeEcdsa,
    /// The signer puts a signature into the program's witness, as [`ProgramSignature`] describes.
    Program(ProgramSignature),
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

impl ProgramSignature {
    /// A signature over `sighash_all`, put into the witness field `name`.
    ///
    /// `name` is the field's name in the contract without the `witness::` prefix. The signer
    /// replaces the field's whole value with the 64-byte signature.
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            path: Vec::new(),
            message: SigMessage::Sighash,
        }
    }

    /// Puts the signature at `path` inside the witness field, rather than replacing the field.
    ///
    /// Each step goes one level into the field's type: `"Left"` or `"Right"` into that branch of
    /// an `Either`, and a decimal index such as `"0"` into that element of an array or tuple. The
    /// supplied witness value must already take the branches the path names, and the value at
    /// the end of the path must have the signature's type, `[u8; 64]`.
    ///
    /// The signer returns `SignerError::WitnessFieldNotFound` if the program doesn't have the
    /// field, and `SignerError::WitnessInjectError` if the path doesn't fit the field's type or
    /// value. It panics if the program has the field but the witness supplied in the
    /// [`ProgramInput`](super::ProgramInput) doesn't contain it.
    #[must_use]
    pub fn at_path<I>(mut self, path: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.path = path.into_iter().map(|route| route.as_ref().to_string()).collect();

        self
    }

    /// Signs the message that `message` derives from `sighash_all`, instead of `sighash_all`
    /// itself.
    #[must_use]
    pub fn with_message(mut self, message: SigMessage) -> Self {
        self.message = message;

        self
    }

    /// Signs the BIP-340 tagged hash `sha256(sha256(tag) || sha256(tag) || sighash_all)`, as
    /// `with_message(SigMessage::Tagged(tag))` does.
    #[must_use]
    pub fn tagged(self, tag: &str) -> Self {
        self.with_message(SigMessage::Tagged(tag.to_string()))
    }

    /// The witness field the signature goes into.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Where inside the witness field the signature goes, empty for the field itself.
    #[must_use]
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// What the signature covers.
    #[must_use]
    pub fn message(&self) -> &SigMessage {
        &self.message
    }
}

impl From<Option<NativeSignature>> for RequiredSignature {
    fn from(signature: Option<NativeSignature>) -> Self {
        match signature {
            Some(NativeSignature::Ecdsa) => Self::NativeEcdsa,
            None => Self::None,
        }
    }
}

impl From<Option<ProgramSignature>> for RequiredSignature {
    fn from(signature: Option<ProgramSignature>) -> Self {
        signature.map_or(Self::None, Self::Program)
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
        let signature = ProgramSignature::new("SIGNATURE")
            .at_path(["Left", "1"])
            .tagged("SimplexTag");

        assert_eq!(signature.name(), "SIGNATURE");
        assert_eq!(signature.path(), ["Left", "1"]);
        assert_eq!(
            signature.message().digest(SIGHASH),
            tagged_hash("SimplexTag", &SIGHASH).to_byte_array()
        );
    }
}
