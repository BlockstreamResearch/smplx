use simplex::alias::*;
use simplex::either::Either;
use simplex::program::Program;

use simplex::simplicityhl::elements::{
    AssetId,
    secp256k1_zkp::{self, Generator, PedersenCommitment, Secp256k1, SecretKey},
};
use simplex::simplicityhl::{Arguments, WitnessValues};

const TEST_SECRET: [u8; 32] = [1; 32];
const ASSET_BYTES: [u8; 32] = [0x42; 32];
const MESSAGE_BYTES: [u8; 32] = [0x24; 32];
const OTHER_MESSAGE_BYTES: [u8; 32] = [0x37; 32];
const MESSAGE64_BYTES: [u8; 64] = [0x22; 64];
const SIGNATURE_BYTES: [u8; 64] = [0x33; 64];
const NONCE_BYTES: [u8; 32] = [0x55; 32];
const OTHER_NONCE_BYTES: [u8; 32] = [0x66; 32];
const WORD_BYTES: [u8; 32] = [0xff; 32];
const TEST_HEIGHT: u32 = 123;
const CONTEXT_INPUT: &[u8] = b"alias container context";
const TEST_U1: u8 = 1;
const TEST_U2: u8 = 3;
const TEST_U4: u8 = 15;
const TEST_ARRAY: [u8; 3] = [1, 2, 3];

#[derive(Clone)]
pub struct AliasTypesProgram(Program);

impl AliasTypesProgram {
    pub fn new(arguments: impl Into<Arguments>) -> Self {
        Self(Program::new(
            derived_alias_types::ALIAS_TYPES_CONTRACT_SOURCE,
            arguments,
        ))
    }
}

impl AsRef<Program> for AliasTypesProgram {
    fn as_ref(&self) -> &Program {
        &self.0
    }
}

simplex::include_simf!("../../../../crates/simplex/tests/ui_simfs/alias_types.simf");

fn main() -> Result<(), String> {
    test_e2e_behaviour()?;
    test_e2e_populated_behaviour()?;
    test_default()?;

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    use derived_alias_types::{AliasTypesArguments, AliasTypesWitness, AliasTypesWitnessSafe};

    let safe_witness = AliasTypesWitnessSafe::default();
    let safe_witness_converted: AliasTypesWitness = safe_witness.clone().into();
    let safe_witness_wit_values: WitnessValues = safe_witness_converted.clone().into();

    let original_witness = AliasTypesWitness::default();
    assert_eq!(safe_witness_converted, original_witness);

    let witness_values = (&original_witness).into();
    assert_eq!(safe_witness_wit_values, witness_values);
    assert_eq!(WitnessValues::from(&safe_witness), witness_values);
    assert_eq!(WitnessValues::from(safe_witness), witness_values);
    let recovered_witness = AliasTypesWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = AliasTypesArguments::default();
    let arguments_values = (&original_arguments).into();
    let recovered_arguments = AliasTypesArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}

fn populated_safe_witness(confidential: bool) -> Result<derived_alias_types::AliasTypesWitnessSafe, String> {
    use derived_alias_types::AliasTypesWitnessSafe;

    let secp = Secp256k1::new();
    let secret = SecretKey::from_slice(&TEST_SECRET).map_err(|error| error.to_string())?;
    let public_key = secp256k1_zkp::PublicKey::from_secret_key(&secp, &secret);
    let pubkey = Pubkey::from(public_key);
    let point = Point::try_from(public_key).map_err(|error| error.to_string())?;
    let ge = Ge::try_from(point).map_err(|error| error.to_string())?;
    let gej = Gej::from_affine(ge).map_err(|error| error.to_string())?;
    let scalar = Scalar::from(secp256k1_zkp::Scalar::ONE);
    let asset = ExplicitAsset(ASSET_BYTES);
    let amount = ExplicitAmount(u64::MAX);
    let generator = Generator::new_unblinded(&secp, AssetId::from(asset).into_tag());
    let commitment = PedersenCommitment::new_unblinded(&secp, amount.0, generator);
    let ctx = Ctx8::from_bytes(CONTEXT_INPUT).map_err(|error| error.to_string())?;
    let explicit_nonce = ExplicitNonce(NONCE_BYTES);
    let other_nonce = ExplicitNonce(OTHER_NONCE_BYTES);
    let outpoint = Outpoint {
        txid: MESSAGE_BYTES,
        vout: u32::MAX,
    };
    let height = Height(TEST_HEIGHT);

    let asset1 = if confidential {
        Asset1::from(generator)
    } else {
        Asset1::from_explicit(asset)
    };

    let amount1 = if confidential {
        Amount1::from(commitment)
    } else {
        Amount1::from(amount)
    };

    let nonce = if confidential {
        Nonce::from(public_key)
    } else {
        Nonce::from_explicit(explicit_nonce)
    };

    let token_amount1 = if confidential {
        TokenAmount1::from_confidential(commitment)
    } else {
        TokenAmount1::from_explicit(amount)
    };

    Ok(AliasTypesWitnessSafe {
        ctx8: ctx.clone(),
        pubkey,
        message: Message(MESSAGE_BYTES),
        message64: Message64(MESSAGE64_BYTES),
        signature: Signature(SIGNATURE_BYTES),
        scalar,
        fe: Fe::ONE,
        ge,
        gej,
        point,
        height,
        time: Time::default(),
        distance: Distance(u16::MAX),
        duration: Duration(u16::MAX),
        lock: Lock(u32::MAX),
        outpoint,
        confidential1: Confidential1::from(commitment),
        explicit_asset: asset,
        asset1,
        explicit_amount: amount,
        amount1,
        explicit_nonce,
        nonce,
        token_amount1,
        bool: confidential,
        u1: TEST_U1,
        u2: TEST_U2,
        u4: TEST_U4,
        u8: u8::MAX,
        u16: u16::MAX,
        u32: u32::MAX,
        u64: u64::MAX,
        u128: u128::MAX,
        u256: WORD_BYTES,
        unit: (),
        tuple: (u8::MAX, u16::MAX),
        array: TEST_ARRAY,
        list: TEST_ARRAY.to_vec(),
        option: Some(u16::MAX),
        either: if confidential {
            Either::Left((Either::Left(TEST_U1), u8::MAX))
        } else {
            Either::Right((u16::MAX, u32::MAX))
        },
        alias_tuple: (pubkey, (height, u8::MAX)),
        alias_single: (scalar,),
        alias_array: [Either::Left(asset1), Either::Right(amount1)],
        alias_list: vec![
            (ctx.clone(), [explicit_nonce, other_nonce]),
            (Ctx8::default(), [other_nonce, explicit_nonce]),
        ],
        alias_option: Some((outpoint, [Message(MESSAGE_BYTES), Message(OTHER_MESSAGE_BYTES)])),
        alias_either: if confidential {
            Either::Left((Some(token_amount1), u16::MAX))
        } else {
            Either::Right(vec![(nonce, false), (nonce, true)])
        },
    })
}

fn test_e2e_populated_behaviour() -> Result<(), String> {
    use derived_alias_types::AliasTypesWitness;

    let program = AliasTypesProgram::new(Arguments::default());
    let compiled = program.as_ref().load().map_err(|error| error.to_string())?;

    for confidential in [true, false] {
        let safe_witness = populated_safe_witness(confidential)?;

        let original_witness: AliasTypesWitness = (&safe_witness).into();
        assert_eq!(AliasTypesWitness::from(safe_witness.clone()), original_witness);

        let witness_values = (&original_witness).into();
        assert_eq!(WitnessValues::from(&safe_witness), witness_values);
        assert_eq!(WitnessValues::from(safe_witness), witness_values);

        let recovered_witness = AliasTypesWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);
        assert_eq!(WitnessValues::from(recovered_witness), witness_values);

        compiled.satisfy(witness_values)?;
    }

    Ok(())
}

fn test_default() -> Result<(), String> {
    use derived_alias_types::{AliasTypesArguments, AliasTypesWitness, AliasTypesWitnessSafe};

    assert_eq!(AliasTypesWitnessSafe::default(), AliasTypesWitnessSafe::default());
    assert_eq!(AliasTypesWitness::default(), AliasTypesWitness::default());
    assert_eq!(AliasTypesArguments::default(), AliasTypesArguments::default());
    Ok(())
}
