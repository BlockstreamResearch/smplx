//! Rust representations of the SimplicityHL builtin aliases.
//!
//! These types preserve the alias layouts used by the existing SDK. Byte and
//! coordinate wrappers describe wire values; they do not prove curve membership
//! or that a signature verifies. Null transaction fields are represented outside
//! the non-null aliases, for example as `Option<Nonce>`.
//!
//! Checked constructors validate inputs at creation time; the existing public
//! wire fields remain available for negative fuzz cases. Reexported key and
//! scalar types use secp256k1's own checked constructors.

// Checked constructors live separately so raw wire representations remain available.
mod checked;

use simplicityhl::elements::{
    confidential,
    secp256k1_zkp::{Generator, PedersenCommitment},
};
use simplicityhl::{
    ResolvedType, Value,
    num::{NonZeroPow2Usize, U256},
    types::{BuiltinAlias, TypeConstructible},
    value::ValueConstructible,
};

/// An explicit Elements asset identifier.
pub use simplicityhl::elements::AssetId as ExplicitAsset;
/// An absolute transaction locktime, serialized as an untagged u32.
pub use simplicityhl::elements::LockTime as Lock;
/// A transaction identifier and output index.
pub use simplicityhl::elements::OutPoint as Outpoint;
/// A validated compressed secp256k1 point.
pub use simplicityhl::elements::secp256k1_zkp::PublicKey as Point;
/// A secp256k1 scalar below the curve order, including zero.
pub use simplicityhl::elements::secp256k1_zkp::Scalar;
/// A validated x-only secp256k1 public key.
pub use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey as Pubkey;
/// A BIP340 signature encoded as 64 bytes; verification is separate.
pub use simplicityhl::elements::secp256k1_zkp::schnorr::Signature;

/// Convert a domain value to the existing SDK's structural witness value.
pub trait SimplicityEncode {
    /// The builtin alias represented by this type.
    const ALIAS: BuiltinAlias;
    /// Encode without transaction serialization prefixes or an outer option.
    fn to_value(&self) -> Value;
}

/// A 32-byte message digest.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Message(pub [u8; 32]);

impl SimplicityEncode for Message {
    const ALIAS: BuiltinAlias = BuiltinAlias::Message;
    fn to_value(&self) -> Value {
        word(self.0)
    }
}

/// A fixed 64-byte message, encoded as an array of u8.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Message64(pub [u8; 64]);

impl SimplicityEncode for Message64 {
    const ALIAS: BuiltinAlias = BuiltinAlias::Message64;
    fn to_value(&self) -> Value {
        Value::byte_array(self.0)
    }
}

/// A field-element wire value. Raw construction accepts every 256-bit encoding.
/// Use [`Fe::from_be_bytes`] to require a canonical value below the field prime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fe(pub [u8; 32]);

impl SimplicityEncode for Fe {
    const ALIAS: BuiltinAlias = BuiltinAlias::Fe;
    fn to_value(&self) -> Value {
        word(self.0)
    }
}

/// An absolute block-height wire value, including the full u32 range.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Height(pub u32);

impl SimplicityEncode for Height {
    const ALIAS: BuiltinAlias = BuiltinAlias::Height;
    fn to_value(&self) -> Value {
        Value::u32(self.0)
    }
}

/// A Unix-time wire value, including the zero fallback returned by jets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Time(pub u32);

impl SimplicityEncode for Time {
    const ALIAS: BuiltinAlias = BuiltinAlias::Time;
    fn to_value(&self) -> Value {
        Value::u32(self.0)
    }
}

/// A relative delay measured in blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Distance(pub u16);

impl SimplicityEncode for Distance {
    const ALIAS: BuiltinAlias = BuiltinAlias::Distance;
    fn to_value(&self) -> Value {
        Value::u16(self.0)
    }
}

/// A relative delay measured in units of 512 seconds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Duration(pub u16);

impl SimplicityEncode for Duration {
    const ALIAS: BuiltinAlias = BuiltinAlias::Duration;
    fn to_value(&self) -> Value {
        Value::u16(self.0)
    }
}

/// An explicit quantity in base units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExplicitAmount(pub u64);

impl SimplicityEncode for ExplicitAmount {
    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitAmount;
    fn to_value(&self) -> Value {
        Value::u64(self.0)
    }
}

/// An explicit 32-byte output nonce or reissuance blinding payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ExplicitNonce(pub [u8; 32]);

impl SimplicityEncode for ExplicitNonce {
    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitNonce;
    fn to_value(&self) -> Value {
        word(self.0)
    }
}

/// Affine coordinates, encoded as (x, y).
/// Raw fields allow off-curve inputs; [`Ge::from_coordinates`] validates curve
/// membership. Converting a [`Point`] also produces valid coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Ge {
    /// Big-endian x-coordinate.
    pub x: Fe,
    /// Big-endian y-coordinate.
    pub y: Fe,
}

impl From<Point> for Ge {
    fn from(point: Point) -> Self {
        let bytes = point.serialize_uncompressed();
        Self {
            x: Fe(bytes[1..33].try_into().expect("32-byte x")),
            y: Fe(bytes[33..65].try_into().expect("32-byte y")),
        }
    }
}

impl SimplicityEncode for Ge {
    const ALIAS: BuiltinAlias = BuiltinAlias::Ge;
    fn to_value(&self) -> Value {
        Value::tuple([self.x.to_value(), self.y.to_value()])
    }
}

/// Jacobian coordinates, encoded as ((X, Y), Z).
/// A nonzero Z represents affine (X/Z², Y/Z³). Raw fields allow arbitrary inputs;
/// [`Gej::from_affine`] validates an affine point and uses Z = 1.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Gej {
    /// Affine-coordinate components, without a curve-membership guarantee.
    pub ge: Ge,
    /// Projective z-coordinate; zero can represent infinity.
    pub z: Fe,
}

impl SimplicityEncode for Gej {
    const ALIAS: BuiltinAlias = BuiltinAlias::Gej;
    fn to_value(&self) -> Value {
        Value::tuple([self.ge.to_value(), self.z.to_value()])
    }
}

/// The selector bit and x-coordinate of a confidential commitment.
/// The selector is a u1 in Simplicity, rather than a Simplicity boolean.
/// Library serialization uses a prefix byte followed by 32 coordinate bytes:
/// 0x0a/0x0b for generators, 0x08/0x09 for Pedersen commitments, and 0x02/0x03
/// for public keys. Only the prefix's low bit is retained, giving 257 bits.
/// For public keys it describes y parity; commitment prefixes use a different
/// selector convention. [`Confidential1::from_serialized`] checks the encoding
/// with the corresponding secp256k1 parser before extracting the payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Confidential1 {
    /// Low bit of the serialized commitment prefix.
    pub selector: bool,
    /// The 32-byte coordinate payload.
    pub x: [u8; 32],
}

impl SimplicityEncode for Confidential1 {
    const ALIAS: BuiltinAlias = BuiltinAlias::Confidential1;
    fn to_value(&self) -> Value {
        Value::tuple([Value::u1(u8::from(self.selector)), word(self.x)])
    }
}

impl From<Generator> for Confidential1 {
    fn from(value: Generator) -> Self {
        commitment(value.serialize())
    }
}

impl From<PedersenCommitment> for Confidential1 {
    fn from(value: PedersenCommitment) -> Self {
        commitment(value.serialize())
    }
}

impl From<Point> for Confidential1 {
    fn from(value: Point) -> Self {
        commitment(value.serialize())
    }
}

/// Invalid construction of an alias value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AliasError {
    /// Null belongs to an outer Option, not a non-null alias.
    #[error("{0} cannot represent a null transaction field")]
    NullField(&'static str),
    /// A SHA256 context must have fewer than 64 buffered bytes.
    #[error("SHA256 context buffer length {0} must be less than 64")]
    ContextBufferLength(usize),
    /// The pending buffer must match the total byte count modulo 64.
    #[error("SHA256 context buffer length does not match total byte count")]
    ContextLengthMismatch,
    /// SHA256 supports fewer than 2^61 input bytes.
    #[error("SHA256 context byte count must be less than 2^61")]
    ContextLengthOverflow,
    /// A canonical field element must be below the secp256k1 field prime.
    #[error("field element is not below the secp256k1 field prime")]
    InvalidFieldElement,
    /// Affine coordinates must describe a finite secp256k1 point.
    #[error("coordinates do not describe a finite secp256k1 point")]
    InvalidCurvePoint,
    /// The prefix and payload must form a supported compressed encoding.
    #[error("invalid confidential commitment or compressed point encoding")]
    InvalidConfidentialEncoding,
    /// Absolute block heights must be below the timestamp threshold.
    #[error("block height must be below 500000000")]
    InvalidHeight,
    /// A jet time is zero or an absolute timestamp at or above the threshold.
    #[error("time must be zero or at least 500000000")]
    InvalidTime,
}

/// The saved state of an ongoing SHA256 computation over bytes.
///
/// `SimplicityHL` encodes this as `(List<u8, 64>, (u64, u256))`: pending bytes
/// (0–63), the number of complete 64-byte blocks compressed, and the 256-bit
/// SHA256 state. After 100 input bytes these are 36 pending bytes, one compressed
/// block, and the state after processing the first 64 bytes.
///
/// This Rust type stores the total byte count internally; [`Ctx8::to_value`]
/// converts it to the compression count expected by the jets. The C frame
/// decoder reconstructs the total as `(compressionCount << 6) + pending.len()`.
/// The total must remain below 2^61 bytes so SHA256's bit count fits in u64.
///
/// The midstate contains eight 32-bit chaining words in big-endian order,
/// updated after each complete block. It is not a finalized digest.
/// [`Ctx8::default`] uses SHA256's standard initial state. [`Ctx8::from_bytes`]
/// computes a context from actual input; [`Ctx8::from_parts`] preserves a
/// caller-supplied midstate without proving which data produced it.
/// See the [builtin alias reference](https://docs.simplicity-lang.org/simplicityhl-reference/type_alias/).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ctx8 {
    buffer: Vec<u8>,
    length: u64,
    midstate: [u8; 32],
}

impl Ctx8 {
    /// Construct a context from pending bytes, total byte count and big-endian midstate.
    ///
    /// # Errors
    /// Rejects a buffer of 64 or more bytes, a count inconsistent with its length,
    /// or a total byte count at or above 2^61.
    pub fn from_parts(buffer: Vec<u8>, length: u64, midstate: [u8; 32]) -> Result<Self, AliasError> {
        if buffer.len() >= 64 {
            return Err(AliasError::ContextBufferLength(buffer.len()));
        }
        if length >= 1 << 61 {
            return Err(AliasError::ContextLengthOverflow);
        }
        if length % 64 != buffer.len() as u64 {
            return Err(AliasError::ContextLengthMismatch);
        }
        Ok(Self {
            buffer,
            length,
            midstate,
        })
    }

    /// Pending bytes, total byte count and SHA256 midstate.
    #[must_use]
    pub fn parts(&self) -> (&[u8], u64, [u8; 32]) {
        (&self.buffer, self.length, self.midstate)
    }
}

impl Default for Ctx8 {
    fn default() -> Self {
        Self {
            buffer: Vec::new(),
            length: 0,
            // SHA256's eight initial words, serialized in big-endian order.
            midstate: simplicityhl::elements::hashes::sha256::Midstate::SHA256_IV.to_parts().0,
        }
    }
}

impl SimplicityEncode for Ctx8 {
    const ALIAS: BuiltinAlias = BuiltinAlias::Ctx8;
    fn to_value(&self) -> Value {
        Value::tuple([
            Value::list(
                self.buffer.iter().copied().map(Value::u8),
                ResolvedType::u8(),
                NonZeroPow2Usize::new(64).expect("power of two"),
            ),
            Value::tuple([Value::u64(self.length >> 6), word(self.midstate)]),
        ])
    }
}

impl SimplicityEncode for Pubkey {
    const ALIAS: BuiltinAlias = BuiltinAlias::Pubkey;
    fn to_value(&self) -> Value {
        word(self.serialize())
    }
}

impl SimplicityEncode for Signature {
    const ALIAS: BuiltinAlias = BuiltinAlias::Signature;
    fn to_value(&self) -> Value {
        Value::byte_array(*self.as_ref())
    }
}

impl SimplicityEncode for Scalar {
    const ALIAS: BuiltinAlias = BuiltinAlias::Scalar;
    fn to_value(&self) -> Value {
        word(self.to_be_bytes())
    }
}

impl SimplicityEncode for Point {
    const ALIAS: BuiltinAlias = BuiltinAlias::Point;
    fn to_value(&self) -> Value {
        Confidential1::from(*self).to_value()
    }
}

impl SimplicityEncode for ExplicitAsset {
    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitAsset;
    fn to_value(&self) -> Value {
        word(self.to_byte_array())
    }
}

impl SimplicityEncode for Lock {
    const ALIAS: BuiltinAlias = BuiltinAlias::Lock;
    fn to_value(&self) -> Value {
        Value::u32(self.to_consensus_u32())
    }
}

impl SimplicityEncode for Outpoint {
    const ALIAS: BuiltinAlias = BuiltinAlias::Outpoint;
    fn to_value(&self) -> Value {
        Value::tuple([word(self.txid.to_byte_array()), Value::u32(self.vout)])
    }
}

/// A non-null Asset1 value. Confidential is Left; explicit is Right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset1 {
    /// A confidential commitment or point.
    Confidential(Generator),
    /// An explicit payload.
    Explicit(ExplicitAsset),
}

impl TryFrom<confidential::Asset> for Asset1 {
    type Error = AliasError;
    fn try_from(value: confidential::Asset) -> Result<Self, Self::Error> {
        match value {
            confidential::Asset::Null => Err(AliasError::NullField("Asset1")),
            confidential::Asset::Confidential(value) => Ok(Self::Confidential(value)),
            confidential::Asset::Explicit(value) => Ok(Self::Explicit(value)),
        }
    }
}

impl From<Asset1> for confidential::Asset {
    fn from(value: Asset1) -> Self {
        match value {
            Asset1::Confidential(value) => Self::Confidential(value),
            Asset1::Explicit(value) => Self::Explicit(value),
        }
    }
}

impl SimplicityEncode for Asset1 {
    const ALIAS: BuiltinAlias = BuiltinAlias::Asset1;
    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(Confidential1::from(*value).to_value(), ResolvedType::u256()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }
}

/// A non-null Amount1 value. Confidential is Left; explicit is Right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount1 {
    /// A confidential commitment or point.
    Confidential(PedersenCommitment),
    /// An explicit payload.
    Explicit(ExplicitAmount),
}

impl TryFrom<confidential::Value> for Amount1 {
    type Error = AliasError;
    fn try_from(value: confidential::Value) -> Result<Self, Self::Error> {
        match value {
            confidential::Value::Null => Err(AliasError::NullField("Amount1")),
            confidential::Value::Confidential(value) => Ok(Self::Confidential(value)),
            confidential::Value::Explicit(value) => Ok(Self::Explicit(ExplicitAmount(value))),
        }
    }
}

impl From<Amount1> for confidential::Value {
    fn from(value: Amount1) -> Self {
        match value {
            Amount1::Confidential(value) => Self::Confidential(value),
            Amount1::Explicit(value) => Self::Explicit(value.0),
        }
    }
}

impl SimplicityEncode for Amount1 {
    const ALIAS: BuiltinAlias = BuiltinAlias::Amount1;
    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(Confidential1::from(*value).to_value(), ResolvedType::u64()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }
}

/// A non-null Nonce value. Confidential is Left; explicit is Right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nonce {
    /// A confidential commitment or point.
    Confidential(Point),
    /// An explicit payload.
    Explicit(ExplicitNonce),
}

impl TryFrom<confidential::Nonce> for Nonce {
    type Error = AliasError;
    fn try_from(value: confidential::Nonce) -> Result<Self, Self::Error> {
        match value {
            confidential::Nonce::Null => Err(AliasError::NullField("Nonce")),
            confidential::Nonce::Confidential(value) => Ok(Self::Confidential(value)),
            confidential::Nonce::Explicit(value) => Ok(Self::Explicit(ExplicitNonce(value))),
        }
    }
}

impl From<Nonce> for confidential::Nonce {
    fn from(value: Nonce) -> Self {
        match value {
            Nonce::Confidential(value) => Self::Confidential(value),
            Nonce::Explicit(value) => Self::Explicit(value.0),
        }
    }
}

impl SimplicityEncode for Nonce {
    const ALIAS: BuiltinAlias = BuiltinAlias::Nonce;
    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(Confidential1::from(*value).to_value(), ResolvedType::u256()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }
}

/// A non-null `TokenAmount1` value. Confidential is Left; explicit is Right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenAmount1 {
    /// A confidential commitment or point.
    Confidential(PedersenCommitment),
    /// An explicit payload.
    Explicit(ExplicitAmount),
}

impl TryFrom<confidential::Value> for TokenAmount1 {
    type Error = AliasError;
    fn try_from(value: confidential::Value) -> Result<Self, Self::Error> {
        match value {
            confidential::Value::Null => Err(AliasError::NullField("TokenAmount1")),
            confidential::Value::Confidential(value) => Ok(Self::Confidential(value)),
            confidential::Value::Explicit(value) => Ok(Self::Explicit(ExplicitAmount(value))),
        }
    }
}

impl From<TokenAmount1> for confidential::Value {
    fn from(value: TokenAmount1) -> Self {
        match value {
            TokenAmount1::Confidential(value) => Self::Confidential(value),
            TokenAmount1::Explicit(value) => Self::Explicit(value.0),
        }
    }
}

impl SimplicityEncode for TokenAmount1 {
    const ALIAS: BuiltinAlias = BuiltinAlias::TokenAmount1;
    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(Confidential1::from(*value).to_value(), ResolvedType::u64()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }
}

/// Represents a transform into a U256 value which can store aliased type.
fn word(bytes: [u8; 32]) -> Value {
    Value::u256(U256::from_byte_array(bytes))
}

fn commitment(bytes: [u8; 33]) -> Confidential1 {
    Confidential1 {
        selector: bytes[0] & 1 != 0,
        x: bytes[1..].try_into().expect("32-byte payload"),
    }
}
