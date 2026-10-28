use rand_core::Rng;

use simplicityhl::elements::AssetId;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::locktime::LOCK_TIME_THRESHOLD;
use simplicityhl::elements::secp256k1_zkp::{
    Generator, PedersenCommitment, PublicKey, Secp256k1, SecretKey, constants,
};

use super::core::{
    Amount1, Asset1, Confidential1, Ctx8, Distance, Duration, ExplicitAmount, ExplicitAsset, ExplicitNonce, Fe, Ge,
    Gej, Height, Lock, Message, Message64, Nonce, Outpoint, Point, Pubkey, Scalar, Signature, Time, TokenAmount1,
};

#[inline]
fn bytes<const N: usize>(rng: &mut (impl Rng + ?Sized)) -> [u8; N] {
    let mut bytes = [0; N];
    rng.fill_bytes(&mut bytes);
    bytes
}

fn reduce(value: [u8; 32], modulus: [u8; 32]) -> [u8; 32] {
    if value < modulus {
        return value;
    }

    let value_hi = u128::from_be_bytes(value[..16].try_into().expect("16-byte limb"));
    let value_lo = u128::from_be_bytes(value[16..].try_into().expect("16-byte limb"));
    let modulus_hi = u128::from_be_bytes(modulus[..16].try_into().expect("16-byte limb"));
    let modulus_lo = u128::from_be_bytes(modulus[16..].try_into().expect("16-byte limb"));
    let (lo, borrow) = value_lo.overflowing_sub(modulus_lo);
    let hi = value_hi - modulus_hi - u128::from(borrow);

    let mut reduced = [0; 32];
    reduced[..16].copy_from_slice(&hi.to_be_bytes());
    reduced[16..].copy_from_slice(&lo.to_be_bytes());

    reduced
}

fn public_key(rng: &mut (impl Rng + ?Sized)) -> PublicKey {
    let scalar = Scalar::random(rng).0;

    let scalar = if scalar == constants::ZERO {
        constants::ONE
    } else {
        scalar
    };

    let secret = SecretKey::from_slice(&scalar).expect("nonzero canonical scalar");

    PublicKey::from_secret_key(&Secp256k1::new(), &secret)
}

fn generator(rng: &mut (impl Rng + ?Sized)) -> Generator {
    Generator::new_unblinded(&Secp256k1::new(), AssetId::from_byte_array(bytes(rng)).into_tag())
}

// A nonzero amount keeps an unblinded commitment away from infinity.
fn commitment(rng: &mut (impl Rng + ?Sized)) -> PedersenCommitment {
    PedersenCommitment::new_unblinded(secp256k1::SECP256K1, rng.next_u64().max(1), generator(rng))
}

impl ExplicitAsset {
    /// Generate an asset identifier using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(bytes(rng))
    }
}

impl ExplicitNonce {
    /// Generate an explicit nonce using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(bytes(rng))
    }
}

impl Message {
    /// Generate a message digest using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(bytes(rng))
    }
}

impl Message64 {
    /// Generate a 64-byte message using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(bytes(rng))
    }
}

impl Signature {
    /// Generate a random signature encoding, without a verification guarantee.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(bytes(rng))
    }
}

impl Lock {
    /// Generate an absolute locktime using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(rng.next_u32())
    }
}

impl Distance {
    /// Generate a relative block delay using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(u16::from_le_bytes(bytes(rng)))
    }
}

impl Duration {
    /// Generate a relative time delay using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(u16::from_le_bytes(bytes(rng)))
    }
}

impl ExplicitAmount {
    /// Generate an explicit amount using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(rng.next_u64())
    }
}

impl Outpoint {
    /// Generate a transaction identifier and output index using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self {
            txid: bytes(rng),
            vout: rng.next_u32(),
        }
    }
}

impl Fe {
    /// Generate a canonical field element by reducing modulo the field prime.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(reduce(bytes(rng), Self::MODULUS))
    }
}

impl Scalar {
    /// Generate a canonical scalar by reducing modulo the curve order.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(reduce(bytes(rng), constants::CURVE_ORDER))
    }
}

impl Height {
    /// Generate a block height below the transaction timestamp threshold [`LOCK_TIME_THRESHOLD`].
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self(rng.next_u32() % LOCK_TIME_THRESHOLD)
    }
}

impl Time {
    /// Generate a timestamp or the zero fallback returned by time jets.
    ///
    /// According to the [`simplicityhl::elements::locktime::Time::from_consensus`] rules.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        match rng.next_u32() {
            0 => Self(0),
            value => Self(LOCK_TIME_THRESHOLD + (value - 1) % (u32::MAX - LOCK_TIME_THRESHOLD + 1)),
        }
    }
}

impl Point {
    /// Generate a finite point from a valid secret key using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self::from_serialized_parts(public_key(rng).serialize())
    }
}

impl Pubkey {
    /// Generate an x-only public key using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self::from(public_key(rng))
    }
}

impl Ge {
    /// Generate a finite affine point using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        let serialized = public_key(rng).serialize_uncompressed();

        let mut x = [0; 32];
        let mut y = [0; 32];

        x.copy_from_slice(&serialized[1..33]);
        y.copy_from_slice(&serialized[33..]);

        Self { x: Fe(x), y: Fe(y) }
    }
}

impl Gej {
    /// Generate a finite Jacobian point with Z = 1 using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self {
            ge: Ge::random(rng),
            z: Fe::ONE,
        }
    }
}

impl Confidential1 {
    /// Generate a Pedersen commitment using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        Self::from(commitment(rng))
    }
}

impl Ctx8 {
    /// Generate a SHA256 context from at most 255 random input bytes.
    #[allow(
        clippy::missing_panics_doc,
        reason = "the generated input is always below the SHA256 byte-count limit"
    )]
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        let len = bytes::<1>(rng)[0];

        let mut input = vec![0; len.into()];
        rng.fill_bytes(&mut input);

        Self::from_bytes(&input).expect("at most 255 input bytes")
    }
}

impl Asset1 {
    /// Generate a confidential or explicit asset using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        if rng.next_u32() & 1 == 0 {
            Self::from(generator(rng))
        } else {
            Self::from_explicit(ExplicitAsset::random(rng))
        }
    }
}

impl Amount1 {
    /// Generate a confidential or explicit amount using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        if rng.next_u32() & 1 == 0 {
            Self::from(commitment(rng))
        } else {
            Self::from(ExplicitAmount::random(rng))
        }
    }
}

impl Nonce {
    /// Generate a confidential or explicit nonce using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        if rng.next_u32() & 1 == 0 {
            Self::from(public_key(rng))
        } else {
            Self::from_explicit(ExplicitNonce::random(rng))
        }
    }
}

impl TokenAmount1 {
    /// Generate a confidential or explicit token amount using the supplied RNG.
    pub fn random<R: Rng + ?Sized>(rng: &mut R) -> Self {
        if rng.next_u32() & 1 == 0 {
            Self::Confidential(commitment(rng).into())
        } else {
            Self::Explicit(ExplicitAmount::random(rng))
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::fmt::Debug;

    use simplicityhl::elements::secp256k1_zkp::{self, PedersenCommitment, PublicKey, XOnlyPublicKey, constants};
    use simplicityhl::elements::{AssetId, LockTime, OutPoint, confidential};

    use rand::SeedableRng;
    use rand::rngs::StdRng;
    use rand_core::TryRng;

    use crate::alias::{
        Amount1, Asset1, Confidential1, Ctx8, Distance, Duration, ExplicitAmount, ExplicitAsset, ExplicitNonce, Fe, Ge,
        Gej, Height, Lock, Message, Message64, Nonce, Outpoint, Point, Pubkey, Scalar, Signature, SimplicityEncode,
        Time, TokenAmount1,
    };

    struct FixedRng {
        bytes: [u8; 32],
        calls: usize,
        limit: usize,
    }

    impl FixedRng {
        fn record_call(&mut self) {
            self.calls += 1;

            assert!(self.calls <= self.limit, "random generation retried the RNG");
        }
    }

    impl TryRng for FixedRng {
        type Error = Infallible;

        fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
            self.record_call();
            Ok(u32::from_be_bytes(self.bytes[..4].try_into().unwrap()))
        }

        fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
            self.record_call();
            Ok(u64::from_be_bytes(self.bytes[..8].try_into().unwrap()))
        }

        fn try_fill_bytes(&mut self, dst: &mut [u8]) -> Result<(), Self::Error> {
            self.record_call();

            for chunk in dst.chunks_mut(32) {
                chunk.copy_from_slice(&self.bytes[..chunk.len()]);
            }

            Ok(())
        }
    }

    fn sample_manual<T>(
        seed_bytes: [u8; 32],
        expected_rng_calls: usize,
        generate: impl FnOnce(&mut FixedRng) -> T,
    ) -> T {
        let mut rng = FixedRng {
            bytes: seed_bytes,
            calls: 0,
            limit: expected_rng_calls,
        };

        let value = generate(&mut rng);
        assert_eq!(rng.calls, expected_rng_calls);

        value
    }

    fn word(hex: &str) -> [u8; 32] {
        hex::decode(hex).unwrap().try_into().unwrap()
    }

    fn samples<T: Debug + PartialEq + SimplicityEncode>(generate: impl Fn(&mut StdRng) -> T) -> Vec<T> {
        let mut first = StdRng::seed_from_u64(42);
        let mut second = StdRng::seed_from_u64(42);

        (0..32)
            .map(|_| {
                let value = generate(&mut first);

                assert_eq!(value, generate(&mut second));
                assert!(value.to_value().is_of_type(&T::ty()));

                value
            })
            .collect()
    }

    #[test]
    fn explicit_asset_random_is_usable() {
        for value in samples(ExplicitAsset::random) {
            assert_eq!(ExplicitAsset::from(AssetId::from(value)), value);
        }
    }

    #[test]
    fn explicit_nonce_random_is_usable() {
        for value in samples(ExplicitNonce::random) {
            let native = confidential::Nonce::try_from(Nonce::from(value)).unwrap();
            assert_eq!(Nonce::try_from(native).unwrap(), Nonce::Explicit(value));
        }
    }

    #[test]
    fn explicit_amount_random_is_usable() {
        for value in samples(ExplicitAmount::random) {
            let native = confidential::Value::try_from(Amount1::from(value)).unwrap();
            assert_eq!(Amount1::try_from(native).unwrap(), Amount1::Explicit(value));
        }
    }

    #[test]
    fn message_random_is_usable() {
        samples(Message::random);
    }

    #[test]
    fn message64_random_is_usable() {
        samples(Message64::random);
    }

    #[test]
    fn signature_random_is_usable() {
        for value in samples(Signature::random) {
            let native = secp256k1_zkp::schnorr::Signature::try_from(value).unwrap();

            assert_eq!(Signature::from(native), value);
        }
    }

    #[test]
    fn lock_random_is_usable() {
        for value in samples(Lock::random) {
            assert_eq!(Lock::from(LockTime::from(value)), value);
        }
    }

    #[test]
    fn distance_random_is_usable() {
        samples(Distance::random);
    }

    #[test]
    fn duration_random_is_usable() {
        samples(Duration::random);
    }

    #[test]
    fn outpoint_random_is_usable() {
        for value in samples(Outpoint::random) {
            assert_eq!(Outpoint::from(OutPoint::from(value)), value);
        }
    }

    #[test]
    fn fe_random_is_canonical() {
        for value in samples(Fe::random) {
            assert_eq!(Fe::from_be_bytes(value.0).unwrap(), value);
        }
    }

    #[test]
    fn scalar_random_is_canonical() {
        for value in samples(Scalar::random) {
            let native = secp256k1_zkp::Scalar::try_from(value).unwrap();
            assert_eq!(Scalar::from(native), value);
        }
    }

    #[test]
    fn height_random_is_valid() {
        for value in samples(Height::random) {
            assert_eq!(Height::from_consensus(value.0).unwrap(), value);
        }
    }

    #[test]
    fn time_random_is_valid() {
        for value in samples(Time::random) {
            assert_eq!(Time::from_consensus(value.0).unwrap(), value);
        }
    }

    #[test]
    fn point_random_is_valid() {
        for value in samples(Point::random) {
            assert_eq!(Point::try_from(PublicKey::try_from(value).unwrap()).unwrap(), value);
        }
    }

    #[test]
    fn pubkey_random_is_valid() {
        for value in samples(Pubkey::random) {
            assert_eq!(Pubkey::from(XOnlyPublicKey::try_from(value).unwrap()), value);
        }
    }

    #[test]
    fn ge_random_is_on_curve() {
        for value in samples(Ge::random) {
            assert_eq!(Ge::from_coordinates(value.x, value.y).unwrap(), value);
        }
    }

    #[test]
    fn gej_random_is_on_curve() {
        for value in samples(Gej::random) {
            assert_eq!(Gej::from_affine(value.ge).unwrap(), value);
            assert_eq!(value.z, Fe::ONE);
        }
    }

    #[test]
    fn confidential1_random_is_valid() {
        for value in samples(Confidential1::random) {
            let native = PedersenCommitment::from_slice(&value.0).unwrap();
            assert_eq!(Confidential1::from(native), value);
        }
    }

    #[test]
    fn ctx8_random_is_valid() {
        for value in samples(Ctx8::random) {
            let (buffer, length, midstate) = value.parts();
            assert_eq!(Ctx8::from_parts(buffer.to_vec(), length, midstate).unwrap(), value);
        }
    }

    #[test]
    fn asset1_random_is_valid() {
        for value in samples(Asset1::random) {
            let native = confidential::Asset::try_from(value).unwrap();
            assert_eq!(Asset1::try_from(native).unwrap(), value);
        }
    }

    #[test]
    fn amount1_random_is_valid() {
        for value in samples(Amount1::random) {
            let native = confidential::Value::try_from(value).unwrap();
            assert_eq!(Amount1::try_from(native).unwrap(), value);
        }
    }

    #[test]
    fn nonce_random_is_valid() {
        for value in samples(Nonce::random) {
            let native = confidential::Nonce::try_from(value).unwrap();
            assert_eq!(Nonce::try_from(native).unwrap(), value);
        }
    }

    #[test]
    fn token_amount1_random_is_valid() {
        for value in samples(TokenAmount1::random) {
            let native = confidential::Value::try_from(value).unwrap();
            assert_eq!(TokenAmount1::try_from(native).unwrap(), value);
        }
    }

    #[test]
    fn point_random_terminates_with_constant_rng() {
        for bytes in [[0; 32], [0xff; 32]] {
            assert!(PublicKey::try_from(sample_manual(bytes, 1, Point::random)).is_ok());
        }
    }

    #[test]
    fn pubkey_random_avoids_zero_scalar() {
        for bytes in [constants::ZERO, constants::CURVE_ORDER] {
            assert_eq!(sample_manual(bytes, 1, Pubkey::random), Pubkey(constants::GENERATOR_X));
        }

        assert!(XOnlyPublicKey::try_from(sample_manual([0xff; 32], 1, Pubkey::random)).is_ok());
    }

    #[test]
    fn height_random_terminates_with_constant_rng() {
        for bytes in [[0; 32], [0xff; 32]] {
            let value = sample_manual(bytes, 1, Height::random);
            assert_eq!(Height::from_consensus(value.0).unwrap(), value);
        }
    }

    #[test]
    fn time_random_preserves_zero_fallback() {
        assert_eq!(sample_manual([0; 32], 1, Time::random), Time(0));

        let value = sample_manual([0xff; 32], 1, Time::random);
        assert_eq!(Time::from_consensus(value.0).unwrap(), value);
    }

    #[test]
    fn confidential1_random_terminates_with_constant_rng() {
        for bytes in [[0; 32], [0xff; 32]] {
            let value = sample_manual(bytes, 2, Confidential1::random);
            assert!(PedersenCommitment::from_slice(&value.0).is_ok());
        }
    }

    #[test]
    fn asset1_random_covers_both_variants() {
        let confidential = sample_manual([0; 32], 2, Asset1::random);
        let explicit = sample_manual([0xff; 32], 2, Asset1::random);
        assert!(matches!(confidential, Asset1::Confidential(_)));
        assert!(matches!(explicit, Asset1::Explicit(_)));

        assert!(confidential::Asset::try_from(confidential).is_ok());
        assert!(confidential::Asset::try_from(explicit).is_ok());
    }

    #[test]
    fn amount1_random_covers_both_variants() {
        let confidential = sample_manual([0; 32], 3, Amount1::random);
        let explicit = sample_manual([0xff; 32], 2, Amount1::random);

        assert!(matches!(confidential, Amount1::Confidential(_)));
        assert!(matches!(explicit, Amount1::Explicit(_)));

        assert!(confidential::Value::try_from(confidential).is_ok());
        assert!(confidential::Value::try_from(explicit).is_ok());
    }

    #[test]
    fn nonce_random_covers_both_variants() {
        let confidential = sample_manual([0; 32], 2, Nonce::random);
        let explicit = sample_manual([0xff; 32], 2, Nonce::random);

        assert!(matches!(confidential, Nonce::Confidential(_)));
        assert!(matches!(explicit, Nonce::Explicit(_)));

        assert!(confidential::Nonce::try_from(confidential).is_ok());
        assert!(confidential::Nonce::try_from(explicit).is_ok());
    }

    #[test]
    fn token_amount1_random_covers_both_variants() {
        let confidential = sample_manual([0; 32], 3, TokenAmount1::random);
        let explicit = sample_manual([0xff; 32], 2, TokenAmount1::random);

        assert!(matches!(confidential, TokenAmount1::Confidential(_)));
        assert!(matches!(explicit, TokenAmount1::Explicit(_)));

        assert!(confidential::Value::try_from(confidential).is_ok());
        assert!(confidential::Value::try_from(explicit).is_ok());
    }

    #[test]
    fn fe_random_reduces_at_modulus_boundaries() {
        let below = word("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2e");
        let above = word("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc30");
        let max_reduced = word("00000000000000000000000000000000000000000000000000000001000003d0");

        assert_eq!(sample_manual(below, 1, Fe::random), Fe(below));
        assert_eq!(sample_manual(Fe::MODULUS, 1, Fe::random), Fe::ZERO);
        assert_eq!(sample_manual(above, 1, Fe::random), Fe::ONE);
        assert_eq!(sample_manual([0xff; 32], 1, Fe::random), Fe(max_reduced));
    }

    #[test]
    fn scalar_random_reduces_with_limb_borrow() {
        // In connection with Secp256k1 module.
        let below = word("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364140");
        let above = word("fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364142");
        let borrow = word("ffffffffffffffffffffffffffffffff00000000000000000000000000000000");
        let borrow_reduced = word("000000000000000000000000000000004551231950b75fc4402da1732fc9bebf");
        let max_reduced = word("000000000000000000000000000000014551231950b75fc4402da1732fc9bebe");

        assert_eq!(sample_manual(below, 1, Scalar::random), Scalar(below));
        assert_eq!(
            sample_manual(constants::CURVE_ORDER, 1, Scalar::random),
            Scalar(constants::ZERO)
        );
        assert_eq!(sample_manual(above, 1, Scalar::random), Scalar(constants::ONE));
        assert_eq!(sample_manual(borrow, 1, Scalar::random), Scalar(borrow_reduced));
        assert_eq!(sample_manual([0xff; 32], 1, Scalar::random), Scalar(max_reduced));
    }
}
