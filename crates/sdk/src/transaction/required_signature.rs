use std::fmt;
use std::sync::Arc;

use crate::utils::tagged_hash;

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

/// The signature the signer puts on a native input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeSignature {
    /// A native ECDSA (WPKH) signature by the signer.
    Ecdsa,
}

/// A signature the signer injects into a program's witness.
#[derive(Debug, Clone)]
pub struct ProgramSignature {
    name: String,
    path: Vec<String>,
    message: SigMessage,
}

/// The signature an input needs, native or into a program's witness.
#[derive(Debug, Clone)]
pub enum RequiredSignature {
    /// No signature is required.
    None,
    /// A native ECDSA (WPKH) signature is required.
    NativeEcdsa,
    /// A signature injected into the program's witness.
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
    #[must_use]
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            path: Vec::new(),
            message: SigMessage::Sighash,
        }
    }

    /// Puts the signature inside the witness field at `path`, e.g. `["Right", "Left", "0"]`.
    #[must_use]
    pub fn at_path<I>(mut self, path: I) -> Self
    where
        I: IntoIterator,
        I::Item: AsRef<str>,
    {
        self.path = path.into_iter().map(|route| route.as_ref().to_string()).collect();

        self
    }

    /// Signs the message `message` derives from `sighash_all` instead of `sighash_all` itself.
    #[must_use]
    pub fn with_message(mut self, message: SigMessage) -> Self {
        self.message = message;

        self
    }

    /// Signs the BIP-340 tagged hash over `tag` and `sighash_all`.
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
