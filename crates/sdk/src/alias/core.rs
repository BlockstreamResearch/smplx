use simplicityhl::either::Either;
use simplicityhl::elements::confidential;
use simplicityhl::elements::secp256k1_zkp::{Generator, Parity, PedersenCommitment, PublicKey};
use simplicityhl::num::{NonZeroPow2Usize, U256};
use simplicityhl::types::{BuiltinAlias, TypeConstructible};
use simplicityhl::value::ValueConstructible;
use simplicityhl::{ResolvedType, Value};

use super::error::{
    Amount1ConversionError, Asset1ConversionError, Ctx8ConversionError, GeConversionError, NonceConversionError,
    TokenAmount1ConversionError,
};

const ZERO_WIDE_BYTES: [u8; 64] = [0; 64];
const ZERO_COMPRESSED_BYTES: [u8; 33] = [0; 33];

/// Encode a domain value as an SDK witness value using its builtin alias layout.
pub trait SimplicityEncode {
    /// The primitive Rust layout used by the existing generated witness fields.
    type ConversionType;

    /// The builtin alias represented by this wrapper.
    const ALIAS: BuiltinAlias;

    /// Encode without transaction serialization prefixes or an outer option.
    #[must_use]
    fn to_value(&self) -> Value;

    /// Convert to the primitive Rust layout of this alias.
    /// The result matches [`Self::to_value`], without transaction prefixes.
    #[must_use]
    fn to_concrete_type(&self) -> Self::ConversionType;

    /// Return the resolved `SimplicityHL` type, including the builtin alias metadata.
    #[must_use]
    fn ty() -> ResolvedType {
        Self::ALIAS.resolve()
    }
}

/// A raw 32-byte Elements asset identifier.
///
/// Represents [`BuiltinAlias::ExplicitAsset`], encoded as `u256`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExplicitAsset(pub [u8; 32]);

impl SimplicityEncode for ExplicitAsset {
    type ConversionType = [u8; 32];
    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitAsset;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A raw 64-byte BIP340 signature.
/// Conversion to the native signature checks the serialized shape; cryptographic
/// verification requires a message and public key separately.
/// The default is a zeroed encoding placeholder without a verification guarantee.
///
/// Represents [`BuiltinAlias::Signature`], encoded as `[u8; 64]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Signature(pub [u8; 64]);

impl Default for Signature {
    fn default() -> Self {
        Self(ZERO_WIDE_BYTES)
    }
}

impl SimplicityEncode for Signature {
    type ConversionType = [u8; 64];
    const ALIAS: BuiltinAlias = BuiltinAlias::Signature;

    fn to_value(&self) -> Value {
        Value::byte_array(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// An absolute transaction locktime.
/// This type can be interpreted as [`simplicityhl::elements::LockTime`].
///
/// Represents [`BuiltinAlias::Lock`], encoded as `u32`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Lock(pub u32);

impl SimplicityEncode for Lock {
    type ConversionType = u32;
    const ALIAS: BuiltinAlias = BuiltinAlias::Lock;

    fn to_value(&self) -> Value {
        Value::u32(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A transaction identifier and output index.
///
/// Represents [`BuiltinAlias::Outpoint`], encoded as `(u256, u32)`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Outpoint {
    /// The transaction ID in its native byte-array order.
    pub txid: [u8; 32],
    /// The transaction output index.
    pub vout: u32,
}

impl SimplicityEncode for Outpoint {
    type ConversionType = ([u8; 32], u32);
    const ALIAS: BuiltinAlias = BuiltinAlias::Outpoint;

    fn to_value(&self) -> Value {
        Value::tuple([word_into_value(self.txid), Value::u32(self.vout)])
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        (self.txid, self.vout)
    }
}

/// A compressed point represented by y parity and a big-endian x-coordinate.
/// The default is an unvalidated encoding with even parity and a zero x-coordinate.
///
/// Represents [`BuiltinAlias::Point`], encoded as `(u1, u256)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Point {
    /// The y-coordinate parity, encoded as a Simplicity u1: even = 0, odd = 1.
    pub parity: Parity,
    /// The 32-byte big-endian x-coordinate.
    pub x: [u8; 32],
}

impl Default for Point {
    fn default() -> Self {
        Self {
            parity: Parity::Even,
            x: Default::default(),
        }
    }
}

impl Point {
    /// According to the secp256k1 base prefix for a compressed secp256k1 public key.
    pub(crate) const BASE_PREFIX: u8 = 0x02;

    /// Extract the low prefix bit and x-coordinate from a 33-byte encoding.
    ///
    /// The prefix and curve membership are not validated. Commitment encodings
    /// retain their own selector convention.
    #[must_use]
    pub fn from_serialized_parts(bytes: [u8; 33]) -> Self {
        let [prefix, x @ ..] = bytes;
        Self {
            parity: if prefix & 1 == 0 { Parity::Even } else { Parity::Odd },
            x,
        }
    }

    /// Serialize with the compressed public-key prefix and stored parity.
    /// Curve membership is not validated.
    #[must_use]
    pub fn compressed_bytes(&self) -> [u8; 33] {
        self.serialized_with_prefix(Self::BASE_PREFIX)
    }

    /// Serialize with a supplied base prefix and the stored parity or selector bit.
    /// The caller must supply a base prefix with its low bit clear.
    #[must_use]
    pub fn serialized_with_prefix(&self, prefix: u8) -> [u8; 33] {
        let mut bytes = [0; 33];
        bytes[0] = prefix | self.parity.to_u8();
        bytes[1..].copy_from_slice(&self.x);
        bytes
    }
}

impl SimplicityEncode for Point {
    type ConversionType = (u8, [u8; 32]);

    const ALIAS: BuiltinAlias = BuiltinAlias::Point;

    fn to_value(&self) -> Value {
        Value::tuple([Value::u1(self.parity.to_u8()), word_into_value(self.x)])
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        (self.parity.to_u8(), self.x)
    }
}

/// A raw big-endian secp256k1 scalar encoding.
/// [`Scalar::from_be_bytes`] checks that it is below the curve order, including
/// zero. Raw construction remains available for negative fuzz inputs.
///
/// Represents [`BuiltinAlias::Scalar`], represented as `u256`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Scalar(pub [u8; 32]);

impl SimplicityEncode for Scalar {
    type ConversionType = [u8; 32];

    const ALIAS: BuiltinAlias = BuiltinAlias::Scalar;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A raw x-only secp256k1 public-key encoding, represented as `u256`.
/// [`Pubkey::from_bytes`] and conversion to the native key validate the encoding.
/// The default is an unvalidated, zeroed encoding placeholder.
///
/// Represents [`BuiltinAlias::Pubkey`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Pubkey(pub [u8; 32]);

impl SimplicityEncode for Pubkey {
    type ConversionType = [u8; 32];

    const ALIAS: BuiltinAlias = BuiltinAlias::Pubkey;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A fixed 32-byte message.
///
/// Represents [`BuiltinAlias::Message`], encoded as `u256`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Message(pub [u8; 32]);

impl SimplicityEncode for Message {
    type ConversionType = [u8; 32];

    const ALIAS: BuiltinAlias = BuiltinAlias::Message;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A fixed 64-byte message.
///
/// Represents [`BuiltinAlias::Message64`], encoded as an 64 byte array.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Message64(pub [u8; 64]);

impl Default for Message64 {
    fn default() -> Self {
        Self(ZERO_WIDE_BYTES)
    }
}

impl SimplicityEncode for Message64 {
    type ConversionType = [u8; 64];

    const ALIAS: BuiltinAlias = BuiltinAlias::Message64;

    fn to_value(&self) -> Value {
        Value::byte_array(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A field-element value. Raw construction accepts every 256-bit encoding.
///
/// Use [`Fe::from_be_bytes`] to require a canonical value below the field prime.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Fe(pub [u8; 32]);

impl SimplicityEncode for Fe {
    type ConversionType = [u8; 32];

    const ALIAS: BuiltinAlias = BuiltinAlias::Fe;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// An absolute block-height wire value.
/// Raw construction permits the full u32 range. [`Height::from_consensus`]
/// validates the block-height domain below the timestamp threshold.
///
/// Represents [`BuiltinAlias::Height`], encoded as `u32`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Height(pub u32);

impl SimplicityEncode for Height {
    type ConversionType = u32;

    const ALIAS: BuiltinAlias = BuiltinAlias::Height;

    fn to_value(&self) -> Value {
        Value::u32(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A Unix-time wire value including the jets' zero fallback.
/// [`Time::from_consensus`] accepts zero or a timestamp at or above the
/// transaction timestamp threshold. Raw construction permits every `u32`.
///
/// Represents [`BuiltinAlias::Time`], encoded as `u32`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Time(pub u32);

impl SimplicityEncode for Time {
    type ConversionType = u32;

    const ALIAS: BuiltinAlias = BuiltinAlias::Time;

    fn to_value(&self) -> Value {
        Value::u32(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A relative delay measured in blocks.
///
/// Represents [`BuiltinAlias::Distance`], encoded as `u16`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Distance(pub u16);

impl SimplicityEncode for Distance {
    type ConversionType = u16;

    const ALIAS: BuiltinAlias = BuiltinAlias::Distance;

    fn to_value(&self) -> Value {
        Value::u16(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// A relative delay.
///
/// Represents [`BuiltinAlias::Duration`], encoded as `u16`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Duration(pub u16);

impl SimplicityEncode for Duration {
    type ConversionType = u16;

    const ALIAS: BuiltinAlias = BuiltinAlias::Duration;

    fn to_value(&self) -> Value {
        Value::u16(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// An explicit quantity in base units.
///
/// Represents [`BuiltinAlias::ExplicitAmount`], encoded as `u64`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExplicitAmount(pub u64);

impl SimplicityEncode for ExplicitAmount {
    type ConversionType = u64;

    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitAmount;

    fn to_value(&self) -> Value {
        Value::u64(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// An explicit 32-byte output nonce or reissuance blinding payload.
///
/// Represents [`BuiltinAlias::ExplicitNonce`], encoded as a 32-byte array
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ExplicitNonce(pub [u8; 32]);

impl SimplicityEncode for ExplicitNonce {
    type ConversionType = [u8; 32];

    const ALIAS: BuiltinAlias = BuiltinAlias::ExplicitNonce;

    fn to_value(&self) -> Value {
        word_into_value(self.0)
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        self.0
    }
}

/// Affine coordinates.
/// Raw fields allow off-curve inputs; [`Ge::from_coordinates`] validates curve
/// membership. Converting a [`Point`] also produces valid coordinates.
/// The default is the unvalidated coordinate pair `(0, 0)`.
///
/// Represents [`BuiltinAlias::Ge`], encoded as `(x, y)`, where `x` and  `y` are [`Fe`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Ge {
    /// Big-endian x-coordinate, represented as an element in `Fe`.
    pub x: Fe,
    /// Big-endian y-coordinate, represented as an element in `Fe`.
    pub y: Fe,
}

impl SimplicityEncode for Ge {
    type ConversionType = ([u8; 32], [u8; 32]);

    const ALIAS: BuiltinAlias = BuiltinAlias::Ge;

    fn to_value(&self) -> Value {
        Value::tuple([self.x.to_value(), self.y.to_value()])
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        (self.x.to_concrete_type(), self.y.to_concrete_type())
    }
}

impl TryFrom<Point> for Ge {
    type Error = GeConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        let bytes = PublicKey::try_from(point)?.serialize_uncompressed();
        Ok(Self {
            x: Fe(bytes[1..33].try_into().expect("32-byte x")),
            y: Fe(bytes[33..65].try_into().expect("32-byte y")),
        })
    }
}

/// Jacobian coordinates.
/// A nonzero `Z` represents affine `(X/Z², Y/Z³)`. Raw fields allow arbitrary inputs;
/// [`Gej::from_affine`] validates an affine point and uses `Z = 1`.
/// The default has `Z = 0`, representing infinity.
///
/// Represents [`BuiltinAlias::Gej`], encoded as `((X, Y), Z)`, where `X`,`Y` and `Z` are [`Ge`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Gej {
    /// Affine-coordinate components, without a curve-membership guarantee.
    pub ge: Ge,
    /// Projective z-coordinate; zero can represent infinity.
    pub z: Fe,
}

impl SimplicityEncode for Gej {
    type ConversionType = (([u8; 32], [u8; 32]), [u8; 32]);

    const ALIAS: BuiltinAlias = BuiltinAlias::Gej;

    fn to_value(&self) -> Value {
        Value::tuple([self.ge.to_value(), self.z.to_value()])
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        (self.ge.to_concrete_type(), self.z.to_concrete_type())
    }
}

/// A holder for a compressed confidential payload.
///
/// Stores the full 33-byte serialization, but encodes only the prefix's low
/// selector bit and the 32-byte coordinate.
/// The default is an unvalidated, zeroed serialization placeholder.
///
/// Represents [`BuiltinAlias::Confidential1`], encoded as `(u1, u256)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Confidential1(
    /// The serialization prefix followed by the 32-byte coordinate.
    pub [u8; 33],
);

impl Default for Confidential1 {
    fn default() -> Self {
        Self(ZERO_COMPRESSED_BYTES)
    }
}

impl SimplicityEncode for Confidential1 {
    type ConversionType = (u8, [u8; 32]);

    const ALIAS: BuiltinAlias = BuiltinAlias::Confidential1;

    fn to_value(&self) -> Value {
        Point::from_serialized_parts(self.0).to_value()
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        Point::from_serialized_parts(self.0).to_concrete_type()
    }
}

impl From<PedersenCommitment> for Confidential1 {
    fn from(value: PedersenCommitment) -> Self {
        Self(value.serialize())
    }
}

/// The saved state of an ongoing SHA256 computation over bytes.
/// todo: review
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
/// [`Ctx8::default`] is a zeroed encoding placeholder. [`Ctx8::from_bytes`]
/// computes a context from actual input, using SHA256's standard initial state;
/// [`Ctx8::from_parts`] preserves a caller-supplied midstate without proving
/// which data produced it.
/// See the [builtin alias reference](https://docs.simplicity-lang.org/simplicityhl-reference/type_alias/).
///
/// Represents [`BuiltinAlias::Ctx8`], encoded as `(List<u8, 64>, (u64, u256))`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ctx8 {
    buffer: Vec<u8>,
    length: u64,
    midstate: [u8; 32],
}

impl SimplicityEncode for Ctx8 {
    type ConversionType = (Vec<u8>, (u64, [u8; 32]));

    const ALIAS: BuiltinAlias = BuiltinAlias::Ctx8;

    fn to_value(&self) -> Value {
        Value::tuple([
            Value::list(
                self.buffer.iter().copied().map(Value::u8),
                ResolvedType::u8(),
                NonZeroPow2Usize::new(64).expect("power of two"),
            ),
            Value::tuple([Value::u64(self.length >> 6), word_into_value(self.midstate)]),
        ])
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        (self.buffer.clone(), (self.length >> 6, self.midstate))
    }
}

impl Ctx8 {
    /// Construct a context from pending bytes, total byte count and big-endian midstate.
    /// Provides a raw interface for creating the struct.
    /// This struct would work even if values are invalid.
    ///
    /// # Errors
    /// Rejects only if a buffer of 64 or more bytes, regarding the simplicity `Ctx8::ALIAS` definition.
    pub fn try_from_raw(buffer: Vec<u8>, length: u64, midstate: [u8; 32]) -> Result<Self, Ctx8ConversionError> {
        if buffer.len() >= 64 {
            return Err(Ctx8ConversionError::ContextBufferLength(buffer.len()));
        }

        Ok(Self {
            buffer,
            length,
            midstate,
        })
    }

    /// Construct a context from pending bytes, total byte count and big-endian midstate.
    /// Check buffer and length invariants required by the Simplicity SHA256 jets.
    /// The supplied midstate is accepted without verifying which input produced it.
    ///
    /// # Errors
    /// Rejects a buffer of 64 or more bytes, a count inconsistent with its length,
    /// or a total byte count at or above 2^61.
    pub fn from_parts(buffer: Vec<u8>, length: u64, midstate: [u8; 32]) -> Result<Self, Ctx8ConversionError> {
        // Complete 64-byte blocks are already compressed into the midstate.
        // The buffer contains only the remaining 0–63 bytes.
        if buffer.len() >= 64 {
            return Err(Ctx8ConversionError::ContextBufferLength(buffer.len()));
        }

        // SHA256 finalization encodes the input length in bits in a 64-bit field.
        // Require length * 8 < 2^64, so the byte count must be below 2^61.
        if length >= 1 << 61 {
            return Err(Ctx8ConversionError::ContextLengthOverflow);
        }

        // The total byte count includes both compressed blocks and pending bytes:
        // length = compressed_blocks * 64 + buffer.len().
        if length % 64 != buffer.len() as u64 {
            return Err(Ctx8ConversionError::ContextLengthMismatch);
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

/// A non-null Elements asset.
///
/// Represents [`BuiltinAlias::Asset1`], encoded as `Either<Confidential1, ExplicitAsset>`
/// The default is the confidential branch with a zeroed encoding placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Asset1 {
    /// The raw asset commitment selector and x-coordinate.
    Confidential(Confidential1),
    /// An explicit payload.
    Explicit(ExplicitAsset),
}

impl Default for Asset1 {
    fn default() -> Self {
        Self::Confidential(Confidential1::default())
    }
}

impl TryFrom<confidential::Asset> for Asset1 {
    type Error = Asset1ConversionError;
    fn try_from(value: confidential::Asset) -> Result<Self, Self::Error> {
        match value {
            confidential::Asset::Null => Err(Asset1ConversionError::NullField),
            confidential::Asset::Confidential(value) => Ok(Self::from(value)),
            confidential::Asset::Explicit(value) => Ok(Self::Explicit(ExplicitAsset::from(value))),
        }
    }
}

impl TryFrom<Asset1> for confidential::Asset {
    type Error = Asset1ConversionError;

    fn try_from(value: Asset1) -> Result<Self, Self::Error> {
        match value {
            Asset1::Confidential(value) => {
                Generator::from_slice(&Point::from_serialized_parts(value.0).serialized_with_prefix(Asset1::GEN_PREFIX))
                    .map(Self::Confidential)
                    .map_err(|_| Asset1ConversionError::InvalidConfidentialEncoding)
            }
            Asset1::Explicit(value) => Ok(Self::Explicit(value.into())),
        }
    }
}

impl SimplicityEncode for Asset1 {
    type ConversionType = Either<(u8, [u8; 32]), [u8; 32]>;

    const ALIAS: BuiltinAlias = BuiltinAlias::Asset1;

    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(value.to_value(), ResolvedType::u256()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        match self {
            Self::Confidential(value) => Either::Left(value.to_concrete_type()),
            Self::Explicit(value) => Either::Right(value.to_concrete_type()),
        }
    }
}

/// A non-null quantity, encoded as `Either<Confidential1, ExplicitAmount>`.
///
/// Represents [`BuiltinAlias::Amount1`], encoded as `Either<Confidential1, ExplicitAmount>`
/// The default is the confidential branch with a zeroed encoding placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Amount1 {
    /// The raw amount commitment selector and x-coordinate.
    Confidential(Confidential1),
    /// An explicit payload.
    Explicit(ExplicitAmount),
}

impl Default for Amount1 {
    fn default() -> Self {
        Self::Confidential(Confidential1::default())
    }
}

impl TryFrom<confidential::Value> for Amount1 {
    type Error = Amount1ConversionError;
    fn try_from(value: confidential::Value) -> Result<Self, Self::Error> {
        match value {
            confidential::Value::Null => Err(Amount1ConversionError::NullField),
            confidential::Value::Confidential(value) => Ok(Self::from(value)),
            confidential::Value::Explicit(value) => Ok(Self::Explicit(ExplicitAmount(value))),
        }
    }
}

impl TryFrom<Amount1> for confidential::Value {
    type Error = Amount1ConversionError;

    fn try_from(value: Amount1) -> Result<Self, Self::Error> {
        match value {
            Amount1::Confidential(value) => PedersenCommitment::from_slice(
                &Point::from_serialized_parts(value.0).serialized_with_prefix(Amount1::CONF_PREFIX),
            )
            .map(Self::Confidential)
            .map_err(|_| Amount1ConversionError::InvalidConfidentialEncoding),
            Amount1::Explicit(value) => Ok(Self::Explicit(value.0)),
        }
    }
}

impl SimplicityEncode for Amount1 {
    type ConversionType = Either<(u8, [u8; 32]), u64>;

    const ALIAS: BuiltinAlias = BuiltinAlias::Amount1;

    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(value.to_value(), ResolvedType::u64()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        match self {
            Self::Confidential(value) => Either::Left(value.to_concrete_type()),
            Self::Explicit(value) => Either::Right(value.to_concrete_type()),
        }
    }
}

/// A non-null output nonce, encoded as `Either<Confidential1, ExplicitNonce>`.
///
/// Represents [`BuiltinAlias::Nonce`], encoded as `Either<Confidential1, ExplicitNonce>`
/// The default is the confidential branch with a zeroed encoding placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nonce {
    /// A confidential commitment or point.
    Confidential(Confidential1),
    /// An explicit payload.
    Explicit(ExplicitNonce),
}

impl Default for Nonce {
    fn default() -> Self {
        Self::Confidential(Confidential1::default())
    }
}

impl TryFrom<confidential::Nonce> for Nonce {
    type Error = NonceConversionError;
    fn try_from(value: confidential::Nonce) -> Result<Self, Self::Error> {
        match value {
            confidential::Nonce::Null => Err(NonceConversionError::NullField),
            confidential::Nonce::Confidential(value) => Ok(Self::from(value)),
            confidential::Nonce::Explicit(value) => Ok(Self::Explicit(ExplicitNonce(value))),
        }
    }
}

impl TryFrom<Nonce> for confidential::Nonce {
    type Error = NonceConversionError;

    fn try_from(value: Nonce) -> Result<Self, Self::Error> {
        match value {
            Nonce::Confidential(value) => Ok(Self::Confidential(PublicKey::try_from(Point::from_serialized_parts(
                value.0,
            ))?)),
            Nonce::Explicit(value) => Ok(Self::Explicit(value.0)),
        }
    }
}

impl SimplicityEncode for Nonce {
    type ConversionType = Either<(u8, [u8; 32]), [u8; 32]>;

    const ALIAS: BuiltinAlias = BuiltinAlias::Nonce;

    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(value.to_value(), ResolvedType::u256()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        match self {
            Self::Confidential(value) => Either::Left(value.to_concrete_type()),
            Self::Explicit(value) => Either::Right(value.to_concrete_type()),
        }
    }
}

/// A non-null reissuance-token quantity with the same layout as [`Amount1`].
///
/// Represents [`BuiltinAlias::TokenAmount1`], encoded as `Either<Confidential1, ExplicitAmount>`
/// The default is the confidential branch with a zeroed encoding placeholder.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenAmount1 {
    /// A confidential commitment or point.
    Confidential(Confidential1),
    /// An explicit payload.
    Explicit(ExplicitAmount),
}

impl Default for TokenAmount1 {
    fn default() -> Self {
        Self::Confidential(Confidential1::default())
    }
}

impl TryFrom<confidential::Value> for TokenAmount1 {
    type Error = TokenAmount1ConversionError;
    fn try_from(value: confidential::Value) -> Result<Self, Self::Error> {
        match value {
            confidential::Value::Null => Err(TokenAmount1ConversionError::NullField),
            confidential::Value::Confidential(value) => Ok(Self::Confidential(value.into())),
            confidential::Value::Explicit(value) => Ok(Self::Explicit(ExplicitAmount(value))),
        }
    }
}

impl TryFrom<TokenAmount1> for confidential::Value {
    type Error = TokenAmount1ConversionError;

    fn try_from(value: TokenAmount1) -> Result<Self, Self::Error> {
        match value {
            TokenAmount1::Confidential(value) => PedersenCommitment::from_slice(
                &Point::from_serialized_parts(value.0).serialized_with_prefix(TokenAmount1::CONF_PREFIX),
            )
            .map(Self::Confidential)
            .map_err(|_| TokenAmount1ConversionError::InvalidConfidentialEncoding),
            TokenAmount1::Explicit(value) => Ok(Self::Explicit(value.0)),
        }
    }
}

impl SimplicityEncode for TokenAmount1 {
    type ConversionType = Either<(u8, [u8; 32]), u64>;

    const ALIAS: BuiltinAlias = BuiltinAlias::TokenAmount1;

    fn to_value(&self) -> Value {
        match self {
            Self::Confidential(value) => Value::left(value.to_value(), ResolvedType::u64()),
            Self::Explicit(value) => Value::right(BuiltinAlias::Confidential1.resolve(), value.to_value()),
        }
    }

    fn to_concrete_type(&self) -> Self::ConversionType {
        match self {
            Self::Confidential(value) => Either::Left(value.to_concrete_type()),
            Self::Explicit(value) => Either::Right(value.to_concrete_type()),
        }
    }
}

/// Represents a transform into a `U256` value which can store aliased type.
fn word_into_value(bytes: [u8; 32]) -> Value {
    Value::u256(U256::from_byte_array(bytes))
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::super::error::{
        Amount1ConversionError, Asset1ConversionError, NonceConversionError, TokenAmount1ConversionError,
    };
    use super::{
        Amount1, Asset1, Confidential1, Ctx8, Distance, Duration, Either, ExplicitAmount, ExplicitAsset, ExplicitNonce,
        Fe, Ge, Gej, Height, Lock, Message, Message64, Nonce, Outpoint, Point, Pubkey, Scalar, Signature,
        SimplicityEncode, Time, TokenAmount1, U256, Value, ValueConstructible,
    };
    use simplicityhl::elements::locktime::LOCK_TIME_THRESHOLD;
    use simplicityhl::elements::secp256k1_zkp::{
        Generator, Parity, PedersenCommitment, PublicKey, Secp256k1, SecretKey, XOnlyPublicKey, constants,
    };
    use simplicityhl::elements::{AssetId, confidential};

    fn assert_default<T>(expected: &T::ConversionType)
    where
        T: Default + SimplicityEncode,
        T::ConversionType: Debug + PartialEq,
    {
        let value = T::default();
        assert_eq!(&value.to_concrete_type(), expected);
        assert!(value.to_value().is_of_type(&T::ty()));
    }

    fn assert_encoding<T>(value: &T, concrete: &T::ConversionType, encoded: &Value)
    where
        T: SimplicityEncode,
        T::ConversionType: Debug + PartialEq,
    {
        assert_eq!(&value.to_concrete_type(), concrete);
        assert_eq!(&value.to_value(), encoded);
        assert!(encoded.is_of_type(&T::ty()));
    }

    fn generator(seed: u8) -> Generator {
        let asset = AssetId::from_byte_array([seed; 32]);
        Generator::new_unblinded(&Secp256k1::new(), asset.into_tag())
    }

    fn commitment(seed: u8) -> PedersenCommitment {
        PedersenCommitment::new_unblinded(&Secp256k1::new(), u64::from(seed), generator(seed))
    }

    fn public_key() -> PublicKey {
        let secret = SecretKey::from_slice(&constants::ONE).unwrap();
        PublicKey::from_secret_key(&Secp256k1::new(), &secret)
    }

    #[test]
    fn explicit_asset_default_matches_layout() {
        assert_default::<ExplicitAsset>(&[0; 32]);
    }

    #[test]
    fn signature_default_matches_layout() {
        assert_default::<Signature>(&[0; 64]);
    }

    #[test]
    fn lock_default_matches_layout() {
        assert_default::<Lock>(&0);
    }

    #[test]
    fn outpoint_default_matches_layout() {
        assert_default::<Outpoint>(&([0; 32], 0));
    }

    #[test]
    fn point_default_matches_layout() {
        assert_default::<Point>(&(0, [0; 32]));

        // Point isn't guaranteed to be valid in after a Default.
        assert!(PublicKey::try_from(Point::default()).is_err());
    }

    #[test]
    fn scalar_default_matches_layout() {
        assert_default::<Scalar>(&[0; 32]);
    }

    #[test]
    fn pubkey_default_matches_layout() {
        assert_default::<Pubkey>(&[0; 32]);

        // Pubkey isn't guaranteed to be valid in after a Default.
        assert!(XOnlyPublicKey::try_from(Pubkey::default()).is_err());
    }

    #[test]
    fn message_default_matches_layout() {
        assert_default::<Message>(&[0; 32]);
    }

    #[test]
    fn message64_default_matches_layout() {
        assert_default::<Message64>(&[0; 64]);
    }

    #[test]
    fn fe_default_matches_layout() {
        assert_default::<Fe>(&[0; 32]);
    }

    #[test]
    fn height_default_matches_layout() {
        assert_default::<Height>(&0);
    }

    #[test]
    fn time_default_matches_layout() {
        assert_default::<Time>(&0);
    }

    #[test]
    fn distance_default_matches_layout() {
        assert_default::<Distance>(&0);
    }

    #[test]
    fn duration_default_matches_layout() {
        assert_default::<Duration>(&0);
    }

    #[test]
    fn explicit_amount_default_matches_layout() {
        assert_default::<ExplicitAmount>(&0);
    }

    #[test]
    fn explicit_nonce_default_matches_layout() {
        assert_default::<ExplicitNonce>(&[0; 32]);
    }

    #[test]
    fn ge_default_matches_layout() {
        assert_default::<Ge>(&([0; 32], [0; 32]));

        // Ge isn't guaranteed to be on the curve after a Default.
        assert!(Ge::from_coordinates(Ge::default().x, Ge::default().y).is_err());
    }

    #[test]
    fn gej_default_matches_layout() {
        assert_default::<Gej>(&(([0; 32], [0; 32]), [0; 32]));
    }

    #[test]
    fn confidential1_default_matches_layout() {
        assert_default::<Confidential1>(&(0, [0; 32]));

        assert_eq!(Confidential1::default().0, [0; 33]);
    }

    #[test]
    fn ctx8_default_matches_layout() {
        assert_default::<Ctx8>(&(Vec::new(), (0, [0; 32])));

        assert_ne!(Ctx8::default(), Ctx8::from_bytes(&[]).unwrap());
    }

    #[test]
    fn asset1_default_matches_layout() {
        assert_default::<Asset1>(&Either::Left((0, [0; 32])));

        assert!(confidential::Asset::try_from(Asset1::default()).is_err());
    }

    #[test]
    fn amount1_default_matches_layout() {
        assert_default::<Amount1>(&Either::Left((0, [0; 32])));

        assert!(confidential::Value::try_from(Amount1::default()).is_err());
    }

    #[test]
    fn nonce_default_matches_layout() {
        assert_default::<Nonce>(&Either::Left((0, [0; 32])));

        assert!(confidential::Nonce::try_from(Nonce::default()).is_err());
    }

    #[test]
    fn token_amount1_default_matches_layout() {
        assert_default::<TokenAmount1>(&Either::Left((0, [0; 32])));

        assert!(confidential::Value::try_from(TokenAmount1::default()).is_err());
    }

    #[test]
    fn explicit_asset_encoding_preserves_bytes() {
        let bytes = constants::GENERATOR_X;
        assert_encoding(
            &ExplicitAsset(bytes),
            &bytes,
            &Value::u256(U256::from_byte_array(bytes)),
        );
    }

    #[test]
    fn outpoint_encoding_preserves_fields() {
        let bytes = constants::GENERATOR_X;
        let value = Outpoint {
            txid: bytes,
            vout: u32::MAX,
        };
        assert_encoding(
            &value,
            &(bytes, u32::MAX),
            &Value::tuple([Value::u256(U256::from_byte_array(bytes)), Value::u32(u32::MAX)]),
        );
    }

    #[test]
    fn scalar_encoding_preserves_bytes() {
        let bytes = constants::ONE;
        assert_encoding(&Scalar(bytes), &bytes, &Value::u256(U256::from_byte_array(bytes)));
    }

    #[test]
    fn pubkey_encoding_preserves_bytes() {
        let bytes = constants::GENERATOR_X;
        assert_encoding(&Pubkey(bytes), &bytes, &Value::u256(U256::from_byte_array(bytes)));
    }

    #[test]
    fn fe_encoding_preserves_bytes() {
        let bytes = constants::GENERATOR_X;
        assert_encoding(&Fe(bytes), &bytes, &Value::u256(U256::from_byte_array(bytes)));
    }

    #[test]
    fn point_encoding_preserves_parity_and_coordinate() {
        let bytes = constants::GENERATOR_X;
        let point = Point {
            parity: Parity::Odd,
            x: bytes,
        };
        assert_encoding(
            &point,
            &(1, bytes),
            &Value::tuple([Value::u1(1), Value::u256(U256::from_byte_array(bytes))]),
        );
    }

    #[test]
    fn gej_encoding_preserves_coordinates() {
        let x = constants::GENERATOR_X;
        let y = constants::GENERATOR_Y;
        let z = constants::ONE;
        let point = Gej {
            ge: Ge { x: Fe(x), y: Fe(y) },
            z: Fe(z),
        };
        assert_encoding(
            &point,
            &((x, y), z),
            &Value::tuple([
                Value::tuple([
                    Value::u256(U256::from_byte_array(x)),
                    Value::u256(U256::from_byte_array(y)),
                ]),
                Value::u256(U256::from_byte_array(z)),
            ]),
        );
    }

    #[test]
    fn message_encoding_preserves_bytes() {
        let bytes = [0x24; 32];
        assert_encoding(&Message(bytes), &bytes, &Value::u256(U256::from_byte_array(bytes)));
    }

    #[test]
    fn message64_encoding_preserves_bytes() {
        let bytes = [0x24; 64];
        assert_encoding(&Message64(bytes), &bytes, &Value::byte_array(bytes));
    }

    #[test]
    fn explicit_nonce_encoding_preserves_bytes() {
        let bytes = [0x55; 32];
        assert_encoding(
            &ExplicitNonce(bytes),
            &bytes,
            &Value::u256(U256::from_byte_array(bytes)),
        );
    }

    #[test]
    fn explicit_amount_encoding_preserves_value() {
        assert_encoding(&ExplicitAmount(u64::MAX), &u64::MAX, &Value::u64(u64::MAX));
    }

    #[test]
    fn height_encoding_preserves_value() {
        assert_encoding(&Height(123), &123, &Value::u32(123));
    }

    #[test]
    fn time_encoding_preserves_value() {
        assert_encoding(
            &Time(LOCK_TIME_THRESHOLD),
            &LOCK_TIME_THRESHOLD,
            &Value::u32(LOCK_TIME_THRESHOLD),
        );
    }

    #[test]
    fn distance_encoding_preserves_value() {
        assert_encoding(&Distance(u16::MAX), &u16::MAX, &Value::u16(u16::MAX));
    }

    #[test]
    fn duration_encoding_preserves_value() {
        assert_encoding(&Duration(u16::MAX), &u16::MAX, &Value::u16(u16::MAX));
    }

    #[test]
    fn ge_conversion_preserves_coordinates() {
        let point = Point::try_from(public_key()).unwrap();
        let ge = Ge::try_from(point).unwrap();

        assert_eq!(ge.to_concrete_type(), (constants::GENERATOR_X, constants::GENERATOR_Y));

        assert_encoding(
            &ge,
            &(constants::GENERATOR_X, constants::GENERATOR_Y),
            &Value::tuple([
                Value::u256(U256::from_byte_array(constants::GENERATOR_X)),
                Value::u256(U256::from_byte_array(constants::GENERATOR_Y)),
            ]),
        );
    }

    #[test]
    fn confidential1_encoding_preserves_selector_and_coordinate() {
        for prefix in [0x08, 0x09, 0x0a, 0x0b] {
            let mut bytes = [0x42; 33];
            bytes[0] = prefix;

            let payload = Confidential1(bytes);

            assert_encoding(
                &payload,
                &(prefix & 1, [0x42; 32]),
                &Value::tuple([Value::u1(prefix & 1), Value::u256(U256::from_byte_array([0x42; 32]))]),
            );
        }

        // Confidential1 serializes correctly
        let native = commitment(1);
        assert_eq!(Confidential1::from(native).0, native.serialize());
    }

    #[test]
    fn ctx8_encoding_uses_compressed_block_count() {
        let context = Ctx8::from_parts(vec![0x42; 36], 100, [0x24; 32]).unwrap();
        let encoded = context.to_value();

        assert_eq!(context.to_concrete_type(), (vec![0x42; 36], (1, [0x24; 32])));

        // Ctx8 doesn't change its values after recovering.
        let mut recovered = context.clone();
        for _ in 0..3 {
            let (buffer, length, midstate) = recovered.parts();
            recovered = Ctx8::from_parts(buffer.to_owned(), length, midstate).unwrap();

            assert_eq!(recovered, context);
            assert_eq!(recovered.to_value(), encoded);
        }
    }

    #[test]
    fn asset1_explicit_conversions_preserve_asset_id() {
        for asset_id_bytes in [constants::ZERO, constants::GENERATOR_X, [0xff; 32]] {
            let explicit_asset = ExplicitAsset(asset_id_bytes);
            let native_asset_id = AssetId::from_byte_array(asset_id_bytes);
            let original_asset = Asset1::from_explicit(explicit_asset);
            let expected_native_asset = confidential::Asset::Explicit(native_asset_id);
            let expected_encoding = original_asset.to_value();

            assert_eq!(Asset1::from(explicit_asset), original_asset);
            assert_eq!(Asset1::from(native_asset_id), original_asset);

            let mut asset_to_convert = original_asset;
            for _ in 0..3 {
                let converted_native_asset = confidential::Asset::try_from(asset_to_convert).unwrap();
                assert_eq!(converted_native_asset, expected_native_asset);

                let recovered_asset = Asset1::try_from(converted_native_asset).unwrap();
                assert_eq!(recovered_asset, original_asset);
                assert_eq!(recovered_asset.to_concrete_type(), Either::Right(asset_id_bytes));
                assert_eq!(recovered_asset.to_value(), expected_encoding);

                asset_to_convert = recovered_asset;
            }
        }
    }

    #[test]
    fn asset1_confidential_conversions_preserve_generator() {
        // Selectors are coming out of Asset1::Confidential structure (u1, [u8; 32])
        let mut observed_selector_bits = [false; 2];
        for seed in 1..=32 {
            let native_generator = generator(seed);
            let serialized_generator = native_generator.serialize();
            let selector_bit = serialized_generator[0] & 1;
            let x_coordinate: [u8; 32] = serialized_generator[1..].try_into().unwrap();

            observed_selector_bits[selector_bit as usize] = true;

            let original_asset = Asset1::from(native_generator);
            let expected_native_asset = confidential::Asset::Confidential(native_generator);
            let expected_concrete_asset = Either::Left((selector_bit, x_coordinate));
            let expected_encoding = original_asset.to_value();

            let mut asset_to_convert = original_asset;
            for _ in 0..3 {
                let converted_native_asset = confidential::Asset::try_from(asset_to_convert).unwrap();
                assert_eq!(converted_native_asset, expected_native_asset);

                let recovered_asset = Asset1::try_from(converted_native_asset).unwrap();
                assert_eq!(recovered_asset, original_asset);
                assert_eq!(recovered_asset.to_concrete_type(), expected_concrete_asset);
                assert_eq!(recovered_asset.to_value(), expected_encoding);

                asset_to_convert = recovered_asset;
            }
        }

        // Check whether we retrieve all kinds of selectors in a Confidential1
        assert_eq!(observed_selector_bits, [true, true]);
    }

    #[test]
    fn amount1_explicit_conversions_preserve_amount() {
        for amount_value in [0, 1, u64::MAX] {
            let explicit_amount = ExplicitAmount(amount_value);
            let original_amount = Amount1::from(explicit_amount);
            let expected_native_amount = confidential::Value::Explicit(amount_value);
            let expected_encoding = original_amount.to_value();

            assert_eq!(Amount1::from(amount_value), original_amount);

            let mut amount_to_convert = original_amount;
            for _ in 0..3 {
                let converted_native_amount = confidential::Value::try_from(amount_to_convert).unwrap();
                assert_eq!(converted_native_amount, expected_native_amount);

                let recovered_amount = Amount1::try_from(converted_native_amount).unwrap();
                assert_eq!(recovered_amount, original_amount);
                assert_eq!(recovered_amount.to_concrete_type(), Either::Right(amount_value));
                assert_eq!(recovered_amount.to_value(), expected_encoding);

                amount_to_convert = recovered_amount;
            }
        }
    }

    #[test]
    fn amount1_confidential_conversions_preserve_commitment() {
        // Selectors are coming out of Amount1::Confidential structure (u1, [u8; 32])
        let mut observed_selector_bits = [false; 2];

        for seed in 1..=32 {
            let native_commitment = commitment(seed);
            let serialized_commitment = native_commitment.serialize();
            let selector_bit = serialized_commitment[0] & 1;
            let x_coordinate: [u8; 32] = serialized_commitment[1..].try_into().unwrap();

            observed_selector_bits[selector_bit as usize] = true;

            let original_amount = Amount1::from(native_commitment);
            let expected_native_amount = confidential::Value::Confidential(native_commitment);
            let expected_concrete_amount = Either::Left((selector_bit, x_coordinate));
            let expected_encoding = original_amount.to_value();

            let mut amount_to_convert = original_amount;
            for _ in 0..3 {
                let converted_native_amount = confidential::Value::try_from(amount_to_convert).unwrap();
                assert_eq!(converted_native_amount, expected_native_amount);

                let recovered_amount = Amount1::try_from(converted_native_amount).unwrap();
                assert_eq!(recovered_amount, original_amount);
                assert_eq!(recovered_amount.to_concrete_type(), expected_concrete_amount);
                assert_eq!(recovered_amount.to_value(), expected_encoding);

                amount_to_convert = recovered_amount;
            }
        }

        // Check whether we retrieve all kinds of selectors in a Confidential1
        assert_eq!(observed_selector_bits, [true, true]);
    }

    #[test]
    fn token_amount1_explicit_conversions_preserve_amount() {
        for amount_value in [0, 1, u64::MAX] {
            let explicit_amount = ExplicitAmount(amount_value);
            let original_token_amount = TokenAmount1::from_explicit(explicit_amount);
            let expected_native_amount = confidential::Value::Explicit(amount_value);
            let expected_encoding = original_token_amount.to_value();

            assert_eq!(TokenAmount1::from(explicit_amount), original_token_amount);
            assert_eq!(TokenAmount1::from(amount_value), original_token_amount);

            let mut token_amount_to_convert = original_token_amount;
            for _ in 0..3 {
                let converted_native_amount = confidential::Value::try_from(token_amount_to_convert).unwrap();
                assert_eq!(converted_native_amount, expected_native_amount);

                let recovered_token_amount = TokenAmount1::try_from(converted_native_amount).unwrap();
                assert_eq!(recovered_token_amount, original_token_amount);
                assert_eq!(recovered_token_amount.to_concrete_type(), Either::Right(amount_value));
                assert_eq!(recovered_token_amount.to_value(), expected_encoding);

                token_amount_to_convert = recovered_token_amount;
            }
        }
    }

    #[test]
    fn token_amount1_confidential_conversions_preserve_commitment() {
        // Selectors are coming out of TokenAmount1::Confidential structure (u1, [u8; 32])
        let mut observed_selector_bits = [false; 2];

        for seed in 1..=32 {
            let native_commitment = commitment(seed);
            let serialized_commitment = native_commitment.serialize();
            let selector_bit = serialized_commitment[0] & 1;
            let x_coordinate: [u8; 32] = serialized_commitment[1..].try_into().unwrap();

            observed_selector_bits[selector_bit as usize] = true;

            let original_token_amount = TokenAmount1::from(native_commitment);
            let expected_native_amount = confidential::Value::Confidential(native_commitment);
            let expected_concrete_amount = Either::Left((selector_bit, x_coordinate));
            let expected_encoding = original_token_amount.to_value();

            let mut token_amount_to_convert = original_token_amount;
            for _ in 0..3 {
                let converted_native_amount = confidential::Value::try_from(token_amount_to_convert).unwrap();
                assert_eq!(converted_native_amount, expected_native_amount);

                let recovered_token_amount = TokenAmount1::try_from(converted_native_amount).unwrap();
                assert_eq!(recovered_token_amount, original_token_amount);
                assert_eq!(recovered_token_amount.to_concrete_type(), expected_concrete_amount);
                assert_eq!(recovered_token_amount.to_value(), expected_encoding);

                token_amount_to_convert = recovered_token_amount;
            }
        }

        // Check whether we retrieve all kinds of selectors in a Confidential1
        assert_eq!(observed_selector_bits, [true, true]);
    }

    #[test]
    fn nonce_conversions_preserve_values() {
        let public_key = public_key();
        let values = [
            Nonce::from(public_key),
            Nonce::from_explicit(ExplicitNonce([0; 32])),
            Nonce::from_explicit(ExplicitNonce(constants::GENERATOR_X)),
        ];

        for original in values {
            let expected_native = match original {
                Nonce::Confidential(_) => confidential::Nonce::Confidential(public_key),
                Nonce::Explicit(nonce) => confidential::Nonce::Explicit(nonce.0),
            };

            let mut recovered = original;

            for _ in 0..3 {
                let native = confidential::Nonce::try_from(recovered).unwrap();
                assert_eq!(native, expected_native);

                recovered = Nonce::try_from(native).unwrap();
                assert_eq!(recovered, original);
                assert_eq!(recovered.to_value(), original.to_value());
            }
        }
    }

    #[test]
    fn asset1_conversion_rejects_null_and_invalid_payloads() {
        assert_eq!(
            Asset1::try_from(confidential::Asset::Null),
            Err(Asset1ConversionError::NullField)
        );

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            for parity in [Parity::Even, Parity::Odd] {
                let point = Point { parity, x };
                let value = Asset1::Confidential(Confidential1(point.serialized_with_prefix(Asset1::GEN_PREFIX)));

                assert_eq!(value.to_concrete_type(), Either::Left((parity.to_u8(), x)));
                assert!(value.to_value().is_of_type(&Asset1::ty()));
                assert_eq!(
                    confidential::Asset::try_from(value),
                    Err(Asset1ConversionError::InvalidConfidentialEncoding)
                );
            }
        }
    }

    #[test]
    fn amount1_conversion_rejects_null_and_invalid_payloads() {
        assert_eq!(
            Amount1::try_from(confidential::Value::Null),
            Err(Amount1ConversionError::NullField)
        );

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            for parity in [Parity::Even, Parity::Odd] {
                let point = Point { parity, x };
                let value = Amount1::Confidential(Confidential1(point.serialized_with_prefix(Amount1::CONF_PREFIX)));

                assert_eq!(value.to_concrete_type(), Either::Left((parity.to_u8(), x)));
                assert!(value.to_value().is_of_type(&Amount1::ty()));
                assert_eq!(
                    confidential::Value::try_from(value),
                    Err(Amount1ConversionError::InvalidConfidentialEncoding)
                );
            }
        }
    }

    #[test]
    fn token_amount1_conversion_rejects_null_and_invalid_payloads() {
        assert_eq!(
            TokenAmount1::try_from(confidential::Value::Null),
            Err(TokenAmount1ConversionError::NullField)
        );

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            for parity in [Parity::Even, Parity::Odd] {
                let point = Point { parity, x };
                let value =
                    TokenAmount1::Confidential(Confidential1(point.serialized_with_prefix(TokenAmount1::CONF_PREFIX)));

                assert_eq!(value.to_concrete_type(), Either::Left((parity.to_u8(), x)));
                assert!(value.to_value().is_of_type(&TokenAmount1::ty()));
                assert_eq!(
                    confidential::Value::try_from(value),
                    Err(TokenAmount1ConversionError::InvalidConfidentialEncoding)
                );
            }
        }
    }

    #[test]
    fn nonce_conversion_rejects_null_and_invalid_payloads() {
        assert_eq!(
            Nonce::try_from(confidential::Nonce::Null),
            Err(NonceConversionError::NullField)
        );

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            let value = Nonce::Confidential(Confidential1(
                Point {
                    parity: Parity::Even,
                    x,
                }
                .compressed_bytes(),
            ));

            assert_eq!(value.to_concrete_type(), Either::Left((0, x)));
            assert!(value.to_value().is_of_type(&Nonce::ty()));
            assert!(confidential::Nonce::try_from(value).is_err());
        }
    }
}
