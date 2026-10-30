use simplicityhl::elements::hashes::{HashEngine, sha256};
use simplicityhl::elements::secp256k1_zkp::{
    self, Generator, Parity, PedersenCommitment, PublicKey, XOnlyPublicKey, constants,
};
use simplicityhl::elements::{AssetId, LockTime, OutPoint, Txid, locktime};

use super::core::{
    Amount1, Asset1, Confidential1, Ctx8, ExplicitAmount, ExplicitAsset, ExplicitNonce, Fe, Ge, Gej, Height, Lock,
    Nonce, Outpoint, Point, Pubkey, Scalar, Signature, Time, TokenAmount1,
};
use super::error::{
    Amount1ConversionError, Asset1ConversionError, Ctx8ConversionError, FeConversionError, GeConversionError,
    GejConversionError, HeightConversionError, NonceConversionError, PointConversionError, PubkeyConversionError,
    ScalarConversionError, SignatureConversionError, TimeConversionError,
};

impl Fe {
    /// secp256k1's field prime p = 2^256 - 2^32 - 977, in big-endian order [`constants::FIELD_SIZE`].
    pub const MODULUS: [u8; 32] = constants::FIELD_SIZE;

    /// The canonical zero field element [`constants::ZERO`].
    pub const ZERO: Self = Self(constants::ZERO);

    /// The canonical multiplicative identity [`constants::ONE`].
    pub const ONE: Self = Self(constants::ONE);

    /// Construct a canonical field element without modular reduction.
    ///
    /// # Errors
    /// Rejects values greater than or equal to [`Fe::MODULUS`]. Zero is valid.
    pub fn from_be_bytes(bytes: [u8; 32]) -> Result<Self, FeConversionError> {
        if bytes >= Self::MODULUS {
            return Err(FeConversionError::InvalidFieldElement);
        }

        Ok(Self(bytes))
    }
}

impl Point {
    /// Construct a finite secp256k1 point from y parity and an x-coordinate.
    ///
    /// # Errors
    /// Rejects a noncanonical coordinate or an x-coordinate with no curve point.
    pub fn from_coordinates(parity: Parity, x: [u8; 32]) -> Result<Self, PointConversionError> {
        let point = Self { parity, x };

        PublicKey::try_from(point)?;

        Ok(point)
    }

    /// Construct a point from an x-only public key and an explicit y parity.
    /// An x-only [`Pubkey`] carries no parity, so the caller selects it.
    ///
    /// # Errors
    /// Returns [`PointConversionError::InvalidCurvePoint`] if the compressed point is invalid.
    pub fn from_pubkey(pubkey: Pubkey, parity: Parity) -> Result<Self, PointConversionError> {
        Self::from_coordinates(parity, pubkey.0)
    }
}

impl TryFrom<[u8; 33]> for Point {
    type Error = PointConversionError;

    fn try_from(bytes: [u8; 33]) -> Result<Self, Self::Error> {
        PublicKey::from_slice(&bytes).map_err(|_| PointConversionError::InvalidCurvePoint)?;
        Ok(Self::from_serialized_parts(bytes))
    }
}

impl TryFrom<PublicKey> for Point {
    type Error = PointConversionError;

    fn try_from(pubkey: PublicKey) -> Result<Self, Self::Error> {
        Self::try_from(pubkey.serialize())
    }
}

impl TryFrom<Point> for PublicKey {
    type Error = PointConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        Self::from_slice(&point.compressed_bytes()).map_err(|_| PointConversionError::InvalidCurvePoint)
    }
}

impl Ge {
    /// Construct a finite affine point using secp256k1's uncompressed-point parser.
    ///
    /// # Errors
    /// Rejects noncanonical coordinates or a point outside the curve.
    pub fn from_coordinates(x: Fe, y: Fe) -> Result<Self, GeConversionError> {
        Fe::from_be_bytes(x.0)?;
        Fe::from_be_bytes(y.0)?;

        let mut bytes = [0; 65];
        bytes[0] = 4;
        bytes[1..33].copy_from_slice(&x.0);
        bytes[33..].copy_from_slice(&y.0);

        PublicKey::from_slice(&bytes).map_err(|_| PointConversionError::InvalidCurvePoint)?;

        Ok(Self { x, y })
    }
}

impl Gej {
    /// Construct a valid finite Jacobian point from affine coordinates, with Z = 1.
    ///
    /// # Errors
    /// Rejects noncanonical or off-curve affine coordinates, including (0, 0).
    pub fn from_affine(ge: Ge) -> Result<Self, GejConversionError> {
        Ok(Self {
            ge: Ge::from_coordinates(ge.x, ge.y)?,
            z: Fe::ONE,
        })
    }

    /// Construct the point at infinity, represented by Z = 0.
    #[must_use]
    pub const fn infinity() -> Self {
        Self {
            ge: Ge {
                x: Fe::ZERO,
                y: Fe::ZERO,
            },
            z: Fe::ZERO,
        }
    }
}

impl TryFrom<Point> for Gej {
    type Error = GejConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        Ok(Self {
            ge: Ge::try_from(point)?,
            z: Fe::ONE,
        })
    }
}

impl Ctx8 {
    /// Compute the unfinalized SHA256 context after consuming the given bytes.
    ///
    /// # Errors
    /// Rejects inputs of 2^61 bytes or more, which overflow SHA256's bit counter.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, Ctx8ConversionError> {
        let length = u64::try_from(bytes.len()).map_err(|_| Ctx8ConversionError::ContextLengthOverflow)?;

        if length >= 1 << 61 {
            return Err(Ctx8ConversionError::ContextLengthOverflow);
        }

        let complete_len = bytes.len() - bytes.len() % 64;
        let mut engine = sha256::HashEngine::new();
        engine.input(&bytes[..complete_len]);

        let midstate = engine
            .midstate()
            .map_err(|_| Ctx8ConversionError::ContextLengthMismatch)?
            .to_parts()
            .0;

        Self::from_parts(bytes[complete_len..].to_vec(), length, midstate)
    }

    /// Construct a jet context from pending bytes, compression count and midstate.
    ///
    /// Calculates automatically consumed length from blocks and passes correct arguments
    /// into [`Ctx8::from_parts`].
    ///
    /// # Errors
    /// Rejects buffers of 64 or more bytes or a total byte count at or above 2^61.
    pub fn from_compressed_parts(
        buffer: Vec<u8>,
        compressed_blocks: u64,
        midstate: [u8; 32],
    ) -> Result<Self, Ctx8ConversionError> {
        if buffer.len() >= 64 {
            return Err(Ctx8ConversionError::ContextBufferLength(buffer.len()));
        }
        let length = compressed_blocks
            .checked_mul(64)
            .and_then(|n| n.checked_add(buffer.len() as u64))
            .ok_or(Ctx8ConversionError::ContextLengthOverflow)?;
        Self::from_parts(buffer, length, midstate)
    }
}

impl TryFrom<(Vec<u8>, u64, [u8; 32])> for Ctx8 {
    type Error = Ctx8ConversionError;

    /// Construct a context from pending bytes, total byte count and midstate.
    ///
    /// # Errors
    /// Returns the same validation errors as [`Ctx8::from_parts`].
    fn try_from((buffer, length, midstate): (Vec<u8>, u64, [u8; 32])) -> Result<Self, Self::Error> {
        Self::from_parts(buffer, length, midstate)
    }
}

impl From<LockTime> for Lock {
    fn from(value: LockTime) -> Self {
        Self(value.to_consensus_u32())
    }
}

impl From<Lock> for LockTime {
    fn from(value: Lock) -> Self {
        Self::from_consensus(value.0)
    }
}

impl Height {
    /// Construct an absolute block height below the transaction timestamp threshold.
    ///
    /// # Errors
    /// Rejects values at or above `500_000_000`. Raw construction remains available.
    pub fn from_consensus(height: u32) -> Result<Self, HeightConversionError> {
        locktime::Height::from_consensus(height).map_err(|_| HeightConversionError::InvalidHeight)?;

        Ok(Self(height))
    }
}

impl From<locktime::Height> for Height {
    fn from(value: locktime::Height) -> Self {
        Height(value.to_consensus_u32())
    }
}

impl Time {
    /// Construct an absolute timestamp or the zero fallback returned by time jets.
    ///
    /// # Errors
    /// Rejects nonzero values below `500_000_000`. Raw construction remains available.
    pub fn from_consensus(time: u32) -> Result<Self, TimeConversionError> {
        if time != 0 {
            locktime::Time::from_consensus(time).map_err(|_| TimeConversionError::InvalidTime)?;
        }

        Ok(Self(time))
    }
}

impl From<locktime::Time> for Time {
    fn from(value: locktime::Time) -> Self {
        Time(value.to_consensus_u32())
    }
}

impl From<AssetId> for ExplicitAsset {
    fn from(value: AssetId) -> Self {
        Self(value.to_byte_array())
    }
}

impl From<ExplicitAsset> for AssetId {
    fn from(value: ExplicitAsset) -> Self {
        Self::from_byte_array(value.0)
    }
}

impl From<OutPoint> for Outpoint {
    fn from(value: OutPoint) -> Self {
        Self {
            txid: value.txid.to_byte_array(),
            vout: value.vout,
        }
    }
}

impl From<Outpoint> for OutPoint {
    fn from(value: Outpoint) -> Self {
        Self {
            txid: Txid::from_byte_array(value.txid),
            vout: value.vout,
        }
    }
}

impl From<secp256k1_zkp::schnorr::Signature> for Signature {
    fn from(value: secp256k1_zkp::schnorr::Signature) -> Self {
        Self(value.serialize())
    }
}

impl TryFrom<&[u8]> for Signature {
    type Error = SignatureConversionError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        secp256k1_zkp::schnorr::Signature::from_slice(bytes)
            .map(Self::from)
            .map_err(|_| SignatureConversionError::InvalidSignature)
    }
}

impl TryFrom<Signature> for secp256k1_zkp::schnorr::Signature {
    type Error = SignatureConversionError;

    fn try_from(value: Signature) -> Result<Self, Self::Error> {
        Self::from_slice(&value.0).map_err(|_| SignatureConversionError::InvalidSignature)
    }
}

impl Scalar {
    /// Construct a canonical scalar from big-endian bytes, including zero.
    ///
    /// # Errors
    /// Rejects values at or above the secp256k1 curve order.
    pub fn from_be_bytes(bytes: [u8; 32]) -> Result<Self, ScalarConversionError> {
        secp256k1_zkp::Scalar::from_be_bytes(bytes)
            .map(Self::from)
            .map_err(|_| ScalarConversionError::InvalidScalar)
    }
}

impl From<secp256k1_zkp::Scalar> for Scalar {
    fn from(value: secp256k1_zkp::Scalar) -> Self {
        Self(value.to_be_bytes())
    }
}

impl TryFrom<Scalar> for secp256k1_zkp::Scalar {
    type Error = ScalarConversionError;

    fn try_from(value: Scalar) -> Result<Self, Self::Error> {
        Self::from_be_bytes(value.0).map_err(|_| ScalarConversionError::InvalidScalar)
    }
}

impl Pubkey {
    /// Construct a validated x-only key from its 32-byte x-coordinate.
    ///
    /// # Errors
    /// Rejects noncanonical coordinates or coordinates without a curve point.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, PubkeyConversionError> {
        XOnlyPublicKey::from_slice(&bytes)
            .map(Self::from)
            .map_err(|_| PubkeyConversionError::InvalidPubkey)
    }
}

impl From<XOnlyPublicKey> for Pubkey {
    fn from(value: XOnlyPublicKey) -> Self {
        Self(value.serialize())
    }
}

impl From<PublicKey> for Pubkey {
    fn from(value: PublicKey) -> Self {
        Self::from(value.x_only_public_key().0)
    }
}

impl TryFrom<Pubkey> for XOnlyPublicKey {
    type Error = PubkeyConversionError;

    fn try_from(value: Pubkey) -> Result<Self, Self::Error> {
        Self::from_slice(&value.0).map_err(|_| PubkeyConversionError::InvalidPubkey)
    }
}

impl TryFrom<Point> for Pubkey {
    type Error = PubkeyConversionError;

    fn try_from(value: Point) -> Result<Self, Self::Error> {
        Ok(Self::from(PublicKey::try_from(value)?))
    }
}

impl Asset1 {
    /// According to the elements conversions of [`Generator`].
    pub(crate) const GEN_PREFIX: u8 = 0x0a;

    /// Construct an explicit asset from its raw identifier payload.
    #[must_use]
    pub const fn from_explicit(asset: ExplicitAsset) -> Self {
        Self::Explicit(asset)
    }

    /// Construct a confidential asset from its raw selector and coordinate.
    /// The point's parity is interpreted as the generator prefix's selector bit.
    ///
    /// # Errors
    /// Rejects coordinates that the native asset-generator parser cannot decode.
    pub fn from_confidential(point: Point) -> Result<Self, Asset1ConversionError> {
        Generator::from_slice(&point.serialized_with_prefix(Self::GEN_PREFIX))
            .map(Self::from)
            .map_err(|_| Asset1ConversionError::InvalidConfidentialEncoding)
    }
}

impl TryFrom<Point> for Asset1 {
    type Error = Asset1ConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        Self::from_confidential(point)
    }
}

impl From<Generator> for Asset1 {
    fn from(value: Generator) -> Self {
        Self::Confidential(Confidential1(value.serialize()))
    }
}

impl From<ExplicitAsset> for Asset1 {
    fn from(value: ExplicitAsset) -> Self {
        Self::from_explicit(value)
    }
}

impl From<AssetId> for Asset1 {
    fn from(value: AssetId) -> Self {
        Self::Explicit(value.into())
    }
}

impl Amount1 {
    /// According to the elements conversions of [`PedersenCommitment`].
    pub(crate) const CONF_PREFIX: u8 = 0x08;

    /// Construct a confidential amount from its raw selector and coordinate.
    /// The point's parity is interpreted as the Pedersen prefix's selector bit.
    ///
    /// # Errors
    /// Rejects coordinates that the native Pedersen commitment parser cannot decode.
    pub fn from_confidential(point: Point) -> Result<Self, Amount1ConversionError> {
        PedersenCommitment::from_slice(&point.serialized_with_prefix(Self::CONF_PREFIX))
            .map(Self::from)
            .map_err(|_| Amount1ConversionError::InvalidConfidentialEncoding)
    }
}

impl TryFrom<Point> for Amount1 {
    type Error = Amount1ConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        Self::from_confidential(point)
    }
}

impl From<PedersenCommitment> for Amount1 {
    fn from(value: PedersenCommitment) -> Self {
        Self::Confidential(value.into())
    }
}

impl From<ExplicitAmount> for Amount1 {
    fn from(value: ExplicitAmount) -> Self {
        Self::Explicit(value)
    }
}

impl From<u64> for Amount1 {
    fn from(value: u64) -> Self {
        Self::Explicit(ExplicitAmount(value))
    }
}

impl TokenAmount1 {
    /// According to the elements conversions of [`PedersenCommitment`].
    pub(crate) const CONF_PREFIX: u8 = 0x08;

    /// Construct a confidential token amount from a validated Pedersen commitment.
    #[must_use]
    pub fn from_confidential(commitment: PedersenCommitment) -> Self {
        Self::Confidential(commitment.into())
    }

    /// Construct an explicit token amount from its quantity payload.
    #[must_use]
    pub const fn from_explicit(amount: ExplicitAmount) -> Self {
        Self::Explicit(amount)
    }
}

impl From<PedersenCommitment> for TokenAmount1 {
    fn from(value: PedersenCommitment) -> Self {
        Self::from_confidential(value)
    }
}

impl From<ExplicitAmount> for TokenAmount1 {
    fn from(value: ExplicitAmount) -> Self {
        Self::from_explicit(value)
    }
}

impl From<u64> for TokenAmount1 {
    fn from(value: u64) -> Self {
        Self::from_explicit(ExplicitAmount(value))
    }
}

impl Nonce {
    /// Construct a confidential nonce from a compressed public-key point.
    ///
    /// # Errors
    /// Rejects noncanonical coordinates or a point outside the curve.
    pub fn from_confidential(point: Point) -> Result<Self, NonceConversionError> {
        Ok(Self::from(PublicKey::try_from(point)?))
    }

    /// Construct an explicit nonce from its raw 32-byte payload.
    #[must_use]
    pub const fn from_explicit(nonce: ExplicitNonce) -> Self {
        Self::Explicit(nonce)
    }
}

impl TryFrom<Point> for Nonce {
    type Error = NonceConversionError;

    fn try_from(point: Point) -> Result<Self, Self::Error> {
        Self::from_confidential(point)
    }
}

impl From<PublicKey> for Nonce {
    fn from(value: PublicKey) -> Self {
        Self::Confidential(Confidential1(value.serialize()))
    }
}

impl From<ExplicitNonce> for Nonce {
    fn from(value: ExplicitNonce) -> Self {
        Self::from_explicit(value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use simplicityhl::Value;
    use simplicityhl::elements::hashes::sha256;
    use simplicityhl::elements::secp256k1_zkp::schnorr::Signature as NativeSignature;
    use simplicityhl::elements::secp256k1_zkp::{
        Generator, Parity, PedersenCommitment, PublicKey, Scalar as NativeScalar, Secp256k1, XOnlyPublicKey, constants,
    };
    use simplicityhl::elements::{AssetId, LockTime, OutPoint, Txid, locktime};
    use simplicityhl::simplicity::jet::{Core, CoreEnv};
    use simplicityhl::simplicity::node::{ConstructNode, CoreConstructible, SimpleFinalizer};
    use simplicityhl::simplicity::{self, BitMachine, types};
    use simplicityhl::value::{StructuralValue, ValueConstructible};

    use crate::alias::{
        AliasError, Amount1, Amount1ConversionError, Asset1, Asset1ConversionError, Ctx8, Ctx8ConversionError,
        ExplicitAsset, Fe, FeConversionError, Ge, GeConversionError, Gej, GejConversionError, Height,
        HeightConversionError, Lock, Nonce, NonceConversionError, Outpoint, Point, PointConversionError, Pubkey,
        PubkeyConversionError, Scalar, ScalarConversionError, Signature, SignatureConversionError, SimplicityEncode,
        Time, TimeConversionError,
    };

    fn run_jet(jet: Core, value: &Value) -> simplicity::Value {
        types::Context::with_context(|ctx| {
            let node = Arc::<ConstructNode<'_>>::jet(&ctx, &jet)
                .finalize_types_non_program()
                .unwrap();
            let program = node.finalize(&mut SimpleFinalizer::new(std::iter::empty())).unwrap();
            let mut machine = BitMachine::for_program(&program).unwrap();
            machine.input(&StructuralValue::from(value).into()).unwrap();
            machine.exec(&program, &CoreEnv::new()).unwrap()
        })
    }

    fn generator_point(parity: Parity) -> Point {
        Point::from_coordinates(parity, constants::GENERATOR_X).unwrap()
    }

    #[test]
    fn alias_error_collects_conversion_errors() {
        fn convert(field: [u8; 32], key: [u8; 32]) -> Result<(Fe, Pubkey), AliasError> {
            Ok((Fe::from_be_bytes(field)?, Pubkey::from_bytes(key)?))
        }

        assert!(convert(Fe::ONE.0, constants::GENERATOR_X).is_ok());
        assert_eq!(
            convert(Fe::MODULUS, constants::GENERATOR_X),
            Err(AliasError::Fe(FeConversionError::InvalidFieldElement))
        );
        assert_eq!(
            convert(Fe::ONE.0, [0; 32]),
            Err(AliasError::Pubkey(PubkeyConversionError::InvalidPubkey))
        );
    }

    #[test]
    fn fe_checked_bounds() {
        let mut largest = Fe::MODULUS;
        largest[31] -= 1;

        for bytes in [constants::ZERO, constants::ONE, largest] {
            assert_eq!(Fe::from_be_bytes(bytes), Ok(Fe(bytes)));
        }

        for bytes in [Fe::MODULUS, [0xff; 32]] {
            assert_eq!(Fe::from_be_bytes(bytes), Err(FeConversionError::InvalidFieldElement));
        }
    }

    #[test]
    fn point_checked_rejects_invalid_encodings() {
        for parity in [Parity::Even, Parity::Odd] {
            for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
                assert_eq!(
                    Point::from_coordinates(parity, x),
                    Err(PointConversionError::InvalidCurvePoint)
                );

                assert!(PublicKey::try_from(Point { parity, x }).is_err());
            }
        }

        // 0x02 is the [`Point::BASE_PREFIX`] for a valid PubKey Secp256k1 EC point.
        for prefix in [0, 1, 4, 8, 9, 10, 11, 255] {
            let mut bytes = generator_point(Parity::Even).compressed_bytes();
            bytes[0] = prefix;
            assert_eq!(Point::try_from(bytes), Err(PointConversionError::InvalidCurvePoint));
        }
    }

    #[test]
    fn point_conversions_preserve_parity_and_coordinate() {
        let key = Pubkey::from_bytes(constants::GENERATOR_X).unwrap();

        for parity in [Parity::Even, Parity::Odd] {
            let expected = generator_point(parity);
            let mut point = Point::from_pubkey(key, parity).unwrap();

            for _ in 0..3 {
                let native = PublicKey::try_from(point).unwrap();
                assert_eq!(native.serialize()[0], Point::BASE_PREFIX | parity.to_u8());

                point = Point::try_from(native.serialize()).unwrap();
                assert_eq!(point, Point::try_from(native).unwrap());
                assert_eq!(point, expected);
                assert_eq!(point.to_concrete_type(), (parity.to_u8(), key.0));
            }
        }
    }

    #[test]
    fn ge_checked_points_are_on_curve() {
        let expected = Ge {
            x: Fe(constants::GENERATOR_X),
            y: Fe(constants::GENERATOR_Y),
        };
        let ge = Ge::from_coordinates(expected.x, expected.y).unwrap();

        assert_eq!(ge, expected);
        assert_eq!(Ge::try_from(generator_point(Parity::Even)).unwrap(), ge);
        assert_eq!(run_jet(Core::GeIsOnCurve, &ge.to_value()), simplicity::Value::u1(1));
        assert_eq!(
            Ge::from_coordinates(Fe::ZERO, Fe::ZERO),
            Err(GeConversionError::InvalidCurvePoint(
                PointConversionError::InvalidCurvePoint
            ))
        );

        for (x, y) in [(Fe(Fe::MODULUS), ge.y), (ge.x, Fe(Fe::MODULUS))] {
            assert_eq!(
                Ge::from_coordinates(x, y),
                Err(GeConversionError::InvalidFieldElement(
                    FeConversionError::InvalidFieldElement
                ))
            );
        }
    }

    #[test]
    fn gej_checked_points_are_on_curve() {
        for parity in [Parity::Even, Parity::Odd] {
            let point = generator_point(parity);
            let ge = Ge::try_from(point).unwrap();
            let gej = Gej::from_affine(ge).unwrap();

            assert_eq!(gej, Gej::try_from(point).unwrap());
            assert_eq!(gej.ge, ge);
            assert_eq!(gej.z, Fe::ONE);
            assert_eq!(run_jet(Core::GejIsOnCurve, &gej.to_value()), simplicity::Value::u1(1));
        }

        assert!(
            Gej::from_affine(Ge {
                x: Fe::ZERO,
                y: Fe::ZERO
            })
            .is_err()
        );

        assert_eq!(
            Gej::try_from(Point::default()),
            Err(GejConversionError::InvalidAffinePoint(
                GeConversionError::InvalidCurvePoint(PointConversionError::InvalidCurvePoint)
            ))
        );

        assert_eq!(
            run_jet(Core::GejIsOnCurve, &Gej::infinity().to_value()),
            simplicity::Value::u1(1)
        );
    }

    #[test]
    fn ctx8_checked_context_finalizes_sha256() {
        for len in [0_usize, 1, 55, 56, 63, 64, 65, 100, 127, 128, 129, 1024] {
            let input: Vec<_> = (0..len).map(|i| u8::try_from(i % 251).unwrap()).collect();
            let expected = Ctx8::from_bytes(&input).unwrap();
            let mut context = expected.clone();

            for _ in 0..3 {
                let (buffer, total, midstate) = context.parts();
                assert_eq!(buffer, &input[len - len % 64..]);
                assert_eq!(total, u64::try_from(len).unwrap());

                context = Ctx8::from_compressed_parts(buffer.to_vec(), total / 64, midstate).unwrap();
                assert_eq!(context, expected);

                assert_eq!(
                    run_jet(Core::Sha256Ctx8Finalize, &context.to_value()),
                    simplicity::Value::u256(sha256::Hash::hash(&input).to_byte_array()),
                    "input length {len}"
                );
            }

            let (buffer, total, midstate) = context.parts();
            assert_eq!(Ctx8::try_from((buffer.to_vec(), total, midstate)).unwrap(), expected);
        }
    }

    #[test]
    fn ctx8_checked_parts_enforce_buffer_and_counter_bounds() {
        let state = Ctx8::default().parts().2;

        assert_eq!(
            Ctx8::from_parts(vec![0; 64], 64, state),
            Err(Ctx8ConversionError::ContextBufferLength(64))
        );

        assert_eq!(
            Ctx8::try_from((vec![0], 0, state)),
            Err(Ctx8ConversionError::ContextLengthMismatch)
        );

        assert_eq!(
            Ctx8::from_parts(vec![], 1 << 61, state),
            Err(Ctx8ConversionError::ContextLengthOverflow)
        );

        for blocks in [1 << 55, u64::MAX] {
            assert_eq!(
                Ctx8::from_compressed_parts(vec![], blocks, state),
                Err(Ctx8ConversionError::ContextLengthOverflow)
            );
        }

        assert_eq!(
            Ctx8::from_compressed_parts(vec![0; 64], 0, state),
            Err(Ctx8ConversionError::ContextBufferLength(64))
        );

        assert!(Ctx8::from_compressed_parts(vec![0; 63], (1 << 55) - 1, state).is_ok());
    }

    #[test]
    fn ctx8_raw_parts_preserve_unchecked_state() {
        let state = [0x42; 32];
        let context = Ctx8::try_from_raw(vec![7], u64::MAX, state).unwrap();

        assert_eq!(context.parts(), (&[7][..], u64::MAX, state));
        assert!(context.to_value().is_of_type(&Ctx8::ty()));
        assert_eq!(
            Ctx8::try_from_raw(vec![0; 64], 64, state),
            Err(Ctx8ConversionError::ContextBufferLength(64))
        );
    }

    #[test]
    fn height_checked_bounds() {
        for value in [0, 1, locktime::LOCK_TIME_THRESHOLD - 1] {
            assert_eq!(Height::from_consensus(value), Ok(Height(value)));

            let native = locktime::Height::from_consensus(value).unwrap();
            let mut height = Height::from(native);

            for _ in 0..3 {
                height = Height::from(locktime::Height::from_consensus(height.0).unwrap());
                assert_eq!(height.0, value);
            }
        }

        for value in [u32::MAX, locktime::LOCK_TIME_THRESHOLD] {
            assert_eq!(Height::from_consensus(value), Err(HeightConversionError::InvalidHeight));
        }
    }

    #[test]
    fn time_checked_bounds_and_zero_fallback() {
        assert_eq!(Time::from_consensus(0), Ok(Time(0)));

        for value in [u32::MAX, locktime::LOCK_TIME_THRESHOLD] {
            let mut time = Time::from_consensus(value).unwrap();

            for _ in 0..3 {
                time = Time::from(locktime::Time::from_consensus(time.0).unwrap());
                assert_eq!(time.0, value);
            }
        }

        for value in [1, locktime::LOCK_TIME_THRESHOLD - 1] {
            assert_eq!(Time::from_consensus(value), Err(TimeConversionError::InvalidTime));
        }
    }

    #[test]
    fn explicit_asset_conversions_preserve_bytes() {
        // `std::array::from_fn` creates an array with 32 bytes.
        for bytes in [[0; 32], [0x42; 32], std::array::from_fn(|i| u8::try_from(i).unwrap())] {
            let mut asset = ExplicitAsset(bytes);

            for _ in 0..3 {
                asset = ExplicitAsset::from(AssetId::from(asset));
                assert_eq!(asset.0, bytes);
                assert_eq!(asset.to_concrete_type(), bytes);
            }
        }
    }

    #[test]
    fn outpoint_conversions_preserve_txid_and_index() {
        let txid: [u8; 32] = std::array::from_fn(|i| u8::try_from(255 - i).unwrap());

        for vout in [0, 1, u32::MAX] {
            let native = OutPoint::new(Txid::from_byte_array(txid), vout);
            let expected = Outpoint { txid, vout };
            let mut outpoint = Outpoint::from(native);

            for _ in 0..3 {
                assert_eq!(OutPoint::from(outpoint), native);
                outpoint = Outpoint::from(OutPoint::from(outpoint));
                assert_eq!(outpoint, expected);
            }
        }
    }

    #[test]
    fn lock_conversions_preserve_height_and_timestamp_values() {
        for value in [
            0,
            1,
            locktime::LOCK_TIME_THRESHOLD - 1,
            locktime::LOCK_TIME_THRESHOLD,
            u32::MAX,
        ] {
            let mut lock = Lock(value);

            for _ in 0..3 {
                lock = Lock::from(LockTime::from(lock));
                assert_eq!(lock.0, value);
            }
        }
    }

    #[test]
    fn signature_checked_length() {
        for len in [0, 1, 32, 63, 65, 128] {
            assert_eq!(
                Signature::try_from(vec![0; len].as_slice()),
                Err(SignatureConversionError::InvalidSignature)
            );
        }
    }

    #[test]
    fn signature_conversions_preserve_all_64_bytes() {
        for bytes in [[0; 64], [0x42; 64], [0xff; 64]] {
            let mut signature = Signature::try_from(bytes.as_slice()).unwrap();

            for _ in 0..3 {
                signature = Signature::from(NativeSignature::try_from(signature).unwrap());
                assert_eq!(signature.0, bytes);
                assert_eq!(signature.to_value(), Value::byte_array(bytes));
            }
        }
    }

    #[test]
    fn scalar_checked_bounds() {
        let mut largest = constants::CURVE_ORDER;
        largest[31] -= 1;

        for bytes in [constants::ZERO, constants::ONE, largest] {
            assert_eq!(Scalar::from_be_bytes(bytes), Ok(Scalar(bytes)));
        }

        for bytes in [constants::CURVE_ORDER, [0xff; 32]] {
            assert_eq!(Scalar::from_be_bytes(bytes), Err(ScalarConversionError::InvalidScalar));
            assert_eq!(
                NativeScalar::try_from(Scalar(bytes)),
                Err(ScalarConversionError::InvalidScalar)
            );
        }
    }

    #[test]
    fn scalar_conversions_preserve_value() {
        for native in [NativeScalar::ZERO, NativeScalar::ONE, NativeScalar::MAX] {
            let expected = native.to_be_bytes();
            let mut scalar = Scalar::from(native);

            for _ in 0..3 {
                scalar = Scalar::from(NativeScalar::try_from(scalar).unwrap());
                assert_eq!(scalar.0, expected);
            }
        }
    }

    #[test]
    fn pubkey_checked_rejects_invalid_coordinates() {
        for bytes in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            assert_eq!(Pubkey::from_bytes(bytes), Err(PubkeyConversionError::InvalidPubkey));
            assert_eq!(
                XOnlyPublicKey::try_from(Pubkey(bytes)),
                Err(PubkeyConversionError::InvalidPubkey)
            );

            assert_eq!(
                Pubkey::try_from(Point {
                    parity: Parity::Even,
                    x: bytes
                }),
                Err(PubkeyConversionError::InvalidCurvePoint(
                    PointConversionError::InvalidCurvePoint
                ))
            );
        }
    }

    #[test]
    fn pubkey_conversions_preserve_x_coordinate() {
        let expected = Pubkey::from_bytes(constants::GENERATOR_X).unwrap();
        let mut pubkey = expected;

        for _ in 0..3 {
            pubkey = Pubkey::from(XOnlyPublicKey::try_from(pubkey).unwrap());
            assert_eq!(pubkey, expected);
        }

        for parity in [Parity::Even, Parity::Odd] {
            let point = generator_point(parity);
            assert_eq!(Pubkey::try_from(point).unwrap(), expected);
            assert_eq!(Pubkey::from(PublicKey::try_from(point).unwrap()), expected);
        }
    }

    #[test]
    fn asset1_checked_accepts_only_valid_generator_points() {
        let generator = Generator::new_unblinded(&Secp256k1::new(), AssetId::from_byte_array([0x42; 32]).into_tag());
        let point = Point::from_serialized_parts(generator.serialize());
        let expected = Asset1::from(generator);

        assert_eq!(Asset1::from_confidential(point).unwrap(), expected);
        assert_eq!(Asset1::try_from(point).unwrap(), expected);

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            for parity in [Parity::Even, Parity::Odd] {
                assert_eq!(
                    Asset1::from_confidential(Point { parity, x }),
                    Err(Asset1ConversionError::InvalidConfidentialEncoding)
                );
            }
        }
    }

    #[test]
    fn amount1_checked_accepts_only_valid_commitment_points() {
        let secp = Secp256k1::new();
        let generator = Generator::new_unblinded(&secp, AssetId::from_byte_array([0x42; 32]).into_tag());
        let commitment = PedersenCommitment::new_unblinded(&secp, 42, generator);
        let point = Point::from_serialized_parts(commitment.serialize());
        let expected = Amount1::from(commitment);

        assert_eq!(Amount1::from_confidential(point).unwrap(), expected);
        assert_eq!(Amount1::try_from(point).unwrap(), expected);

        for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
            for parity in [Parity::Even, Parity::Odd] {
                assert_eq!(
                    Amount1::from_confidential(Point { parity, x }),
                    Err(Amount1ConversionError::InvalidConfidentialEncoding)
                );
            }
        }
    }

    #[test]
    fn nonce_checked_accepts_only_valid_public_key_points() {
        for parity in [Parity::Even, Parity::Odd] {
            let point = generator_point(parity);
            let expected = Nonce::from(PublicKey::try_from(point).unwrap());

            assert_eq!(Nonce::from_confidential(point).unwrap(), expected);
            assert_eq!(Nonce::try_from(point).unwrap(), expected);

            for x in [[0; 32], Fe::MODULUS, [0xff; 32]] {
                assert_eq!(
                    Nonce::from_confidential(Point { parity, x }),
                    Err(NonceConversionError::InvalidCurvePoint(
                        PointConversionError::InvalidCurvePoint
                    ))
                );
            }
        }
    }
}
