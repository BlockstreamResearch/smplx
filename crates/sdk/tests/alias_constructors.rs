use std::sync::Arc;

use simplicityhl::elements::{
    hashes::sha256,
    secp256k1_zkp::{Generator, PedersenCommitment, Secp256k1, SecretKey, constants},
};
use simplicityhl::simplicity::{
    self, BitMachine,
    jet::{Core, CoreEnv},
    node::{ConstructNode, CoreConstructible, SimpleFinalizer},
    types,
};
use simplicityhl::{Value, value::StructuralValue};
use smplx_sdk::alias::*;

fn run_jet(jet: Core, value: Value) -> simplicity::Value {
    types::Context::with_context(|ctx| {
        let node = Arc::<ConstructNode<'_>>::jet(&ctx, &jet)
            .finalize_types_non_program()
            .unwrap();
        let program = node.finalize(&mut SimpleFinalizer::new(std::iter::empty())).unwrap();
        let mut machine = BitMachine::for_program(&program).unwrap();
        machine.input(&StructuralValue::from(&value).into()).unwrap();
        machine.exec(&program, &CoreEnv::new()).unwrap()
    })
}

#[test]
fn canonical_field_boundaries() {
    assert_eq!(Fe::from_be_bytes([0; 32]), Ok(Fe::ZERO));
    let mut largest = Fe::MODULUS;
    largest[31] -= 1;
    assert_eq!(Fe::from_be_bytes(largest), Ok(Fe(largest)));
    assert_eq!(Fe::from_be_bytes(Fe::MODULUS), Err(AliasError::InvalidFieldElement));
    assert!(Fe::from_be_bytes([0xff; 32]).is_err());
    // Scalar's reexport already supplies the corresponding checked constructor.
    assert_eq!(Scalar::from_be_bytes([0; 32]).unwrap(), Scalar::ZERO);
    assert!(Scalar::from_be_bytes([0xff; 32]).is_err());
}

#[test]
fn checked_points_are_on_curve() {
    // Use the library's known generator coordinates instead of duplicating them.
    let generator = Ge::from_coordinates(Fe(constants::GENERATOR_X), Fe(constants::GENERATOR_Y)).unwrap();
    assert_eq!(
        run_jet(Core::GeIsOnCurve, generator.to_value()),
        simplicity::Value::u1(1)
    );
    assert_eq!(
        Ge::from_coordinates(Fe::ZERO, Fe::ZERO),
        Err(AliasError::InvalidCurvePoint)
    );
    assert_eq!(
        Ge::from_coordinates(Fe(Fe::MODULUS), generator.y),
        Err(AliasError::InvalidFieldElement)
    );
    assert!(
        Gej::from_affine(Ge {
            x: Fe::ZERO,
            y: Fe::ZERO
        })
        .is_err()
    );

    let secp = Secp256k1::new();
    for n in 1..=128 {
        let mut bytes = [0; 32];
        bytes[31] = n;
        let point = Point::from_secret_key(&secp, &SecretKey::from_slice(&bytes).unwrap());
        let ge = Ge::from(point);
        assert_eq!(Ge::from_coordinates(ge.x, ge.y).unwrap(), ge);
        let gej = Gej::from_affine(ge).unwrap();
        assert_eq!(gej, Gej::from(point));
        assert_eq!(gej.z, Fe::ONE);
        assert_eq!(run_jet(Core::GejIsOnCurve, gej.to_value()), simplicity::Value::u1(1));
    }
    assert_eq!(
        run_jet(Core::GejIsOnCurve, Gej::infinity().to_value()),
        simplicity::Value::u1(1)
    );
}

#[test]
fn checked_confidential_encodings() {
    let secp = Secp256k1::new();
    let point = Point::from_secret_key(&secp, &SecretKey::from_slice(&constants::ONE).unwrap());
    let asset = ExplicitAsset::from_byte_array([0x42; 32]);
    let generator = Generator::new_unblinded(&secp, asset.into_tag());
    let amount = PedersenCommitment::new_unblinded(&secp, 42, generator);
    assert_eq!(
        Confidential1::from_serialized(point.serialize()).unwrap(),
        Confidential1::from(point)
    );
    assert_eq!(
        Confidential1::from_serialized(generator.serialize()).unwrap(),
        Confidential1::from(generator)
    );
    assert_eq!(
        Confidential1::from_serialized(amount.serialize()).unwrap(),
        Confidential1::from(amount)
    );
    for prefix in [0, 1, 2, 3, 4, 8, 9, 10, 11] {
        let mut invalid = [0; 33];
        invalid[0] = prefix;
        assert_eq!(
            Confidential1::from_serialized(invalid),
            Err(AliasError::InvalidConfidentialEncoding)
        );
    }
    let mut bad_prefix = point.serialize();
    bad_prefix[0] = 4;
    assert!(Confidential1::from_serialized(bad_prefix).is_err());
    let mut noncanonical = point.serialize();
    noncanonical[1..].copy_from_slice(&Fe::MODULUS);
    assert!(Confidential1::from_serialized(noncanonical).is_err());
}

#[test]
fn sha256_context_matches_finalize_jet() {
    // Include both SHA256 padding boundaries and compression-block boundaries.
    for len in [0, 1, 55, 56, 63, 64, 65, 100, 127, 128, 129, 1024] {
        let data: Vec<_> = (0..len).map(|i| (i % 251) as u8).collect();
        let context = Ctx8::from_bytes(&data).unwrap();
        let (pending, total, midstate) = context.parts();
        assert_eq!(pending, &data[len - len % 64..]);
        assert_eq!(total, len as u64);
        assert_eq!(
            Ctx8::from_compressed_parts(pending.to_vec(), total / 64, midstate).unwrap(),
            context
        );
        assert_eq!(
            run_jet(Core::Sha256Ctx8Finalize, context.to_value()),
            simplicity::Value::u256(sha256::Hash::hash(&data).to_byte_array()),
            "length {len}"
        );
    }
    assert_eq!(Ctx8::from_bytes(&[]).unwrap(), Ctx8::default());
    assert_eq!(Ctx8::default().parts().2, sha256::Midstate::SHA256_IV.to_parts().0);
}

#[test]
fn sha256_context_rejects_counter_overflow() {
    let state = Ctx8::default().parts().2;
    assert_eq!(
        Ctx8::from_compressed_parts(vec![0; 64], 0, state),
        Err(AliasError::ContextBufferLength(64))
    );
    assert_eq!(
        Ctx8::from_compressed_parts(vec![], 1 << 55, state),
        Err(AliasError::ContextLengthOverflow)
    );
    assert_eq!(
        Ctx8::from_compressed_parts(vec![], u64::MAX, state),
        Err(AliasError::ContextLengthOverflow)
    );
    assert_eq!(
        Ctx8::from_parts(vec![], 1 << 61, state),
        Err(AliasError::ContextLengthOverflow)
    );
    assert!(Ctx8::from_compressed_parts(vec![0; 63], (1 << 55) - 1, state).is_ok());
}

#[test]
fn checked_locktime_domains_preserve_zero_fallback() {
    assert_eq!(Height::from_consensus(499_999_999).unwrap(), Height(499_999_999));
    assert_eq!(Height::from_consensus(500_000_000), Err(AliasError::InvalidHeight));
    assert_eq!(Time::from_consensus(0).unwrap(), Time(0));
    assert_eq!(Time::from_consensus(499_999_999), Err(AliasError::InvalidTime));
    assert_eq!(Time::from_consensus(500_000_000).unwrap(), Time(500_000_000));
    assert!(Time::from_consensus(u32::MAX).is_ok());
    // Existing raw paths remain available for negative fuzz cases.
    assert_eq!(Height(u32::MAX).0, u32::MAX);
    assert_eq!(Fe([0xff; 32]).0, [0xff; 32]);
}
