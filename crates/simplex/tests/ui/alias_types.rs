use simplex::alias::*;
use simplex::program::Program;
use simplex::simplicityhl::elements::{
    confidential,
    secp256k1_zkp::{Generator, PedersenCommitment, Secp256k1, SecretKey},
};
use simplex::simplicityhl::types::BuiltinAlias;
use simplex::simplicityhl::value::ValueConstructible;
use simplex::simplicityhl::{Arguments, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

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

fn entry<T: SimplicityEncode>(name: &'static str, value: &T) -> (&'static str, BuiltinAlias, Value) {
    (name, T::ALIAS, value.to_value())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let secp = Secp256k1::new();
    let point = Point::from_secret_key(&secp, &SecretKey::from_slice(&[1; 32])?);
    let pubkey = point.x_only_public_key().0;
    let asset = ExplicitAsset::from_byte_array([0x42; 32]);
    let generator = Generator::new_unblinded(&secp, asset.into_tag());
    let commitment = PedersenCommitment::new_unblinded(&secp, 42, generator);
    let ge = Ge::from(point);
    let signature = Signature::from_slice(&[0; 64])?;
    let program = AliasTypesProgram::new(Arguments::default());
    let compiled = program.as_ref().load()?;
    let witness_types = program.as_ref().get_witness_types()?;
    assert_eq!(witness_types.iter().count(), 24);

    // Exercise both branches of every non-null confidential alias. The current
    // macro still generates primitive fields; the SDK types bridge via Value.
    for confidential in [false, true] {
        let asset1 = if confidential {
            Asset1::Confidential(generator)
        } else {
            Asset1::Explicit(asset)
        };
        let amount1 = if confidential {
            Amount1::Confidential(commitment)
        } else {
            Amount1::Explicit(ExplicitAmount(u64::MAX))
        };
        let nonce = if confidential {
            Nonce::Confidential(point)
        } else {
            Nonce::Explicit(ExplicitNonce([0x55; 32]))
        };
        let tokens = if confidential {
            TokenAmount1::Confidential(commitment)
        } else {
            TokenAmount1::Explicit(ExplicitAmount(0))
        };
        let entries = [
            entry("CTX8", &Ctx8::default()),
            entry("PUBKEY", &pubkey),
            entry("MESSAGE", &Message([0x11; 32])),
            entry("MESSAGE64", &Message64([0x22; 64])),
            entry("SIGNATURE", &signature),
            entry("SCALAR", &Scalar::ZERO),
            entry("FE", &Fe([0xff; 32])),
            entry("GE", &ge),
            entry("GEJ", &Gej { ge, z: Fe([0; 32]) }),
            entry("POINT", &point),
            entry("HEIGHT", &Height(u32::MAX)),
            entry("TIME", &Time(0)),
            entry("DISTANCE", &Distance(u16::MAX)),
            entry("DURATION", &Duration(u16::MAX)),
            entry("LOCK", &Lock::from_consensus(u32::MAX)),
            entry("OUTPOINT", &Outpoint::default()),
            entry("CONFIDENTIAL1", &Confidential1::from(generator)),
            entry("EXPLICIT_ASSET", &asset),
            entry("ASSET1", &asset1),
            entry("EXPLICIT_AMOUNT", &ExplicitAmount(u64::MAX)),
            entry("AMOUNT1", &amount1),
            entry("EXPLICIT_NONCE", &ExplicitNonce([0x55; 32])),
            entry("NONCE", &nonce),
            entry("TOKEN_AMOUNT1", &tokens),
        ];
        // Include Ctx8 with the largest permitted pending buffer as well.
        for ctx in [Ctx8::default(), Ctx8::from_parts(vec![0x37; 63], 127, [0x83; 32])?] {
            let mut map = std::collections::HashMap::new();
            for (name, alias, value) in &entries {
                let key = TemplateProgramWitness::witness_from_str(*name);
                assert_eq!(value.ty(), &alias.resolve(), "{name}");
                assert_eq!(witness_types.get(&key), Some(&alias.resolve()), "{name}");
                map.insert(key, value.clone());
            }
            map.insert(TemplateProgramWitness::witness_from_str("CTX8"), ctx.to_value());
            let values = WitnessValues::from_map(map);
            let raw = derived_alias_types::AliasTypesWitness::from_witness(&values)?;
            assert_eq!(WitnessValues::from(&raw), values);
            assert_eq!(raw.pubkey, pubkey.serialize());
            assert_eq!(raw.message64, [0x22; 64]);
            assert_eq!(raw.signature, [0; 64]);
            assert_eq!(
                raw.point,
                (point.serialize()[0] & 1, point.serialize()[1..].try_into()?)
            );
            assert_eq!(
                raw.ge,
                (
                    point.serialize_uncompressed()[1..33].try_into()?,
                    point.serialize_uncompressed()[33..].try_into()?
                )
            );
            assert_eq!(
                raw.confidential1,
                (generator.serialize()[0] & 1, generator.serialize()[1..].try_into()?)
            );
            assert_eq!(raw.explicit_asset, [0x42; 32]);
            assert_eq!(raw.time, 0);
            assert_eq!(raw.explicit_amount, u64::MAX);
            assert_eq!(raw.ctx8.0.len(), ctx.parts().0.len());
            assert_eq!(raw.ctx8.1.0, ctx.parts().1 / 64);
            compiled.satisfy(values)?;
        }
        assert_eq!(Asset1::try_from(confidential::Asset::from(asset1))?, asset1);
        assert_eq!(Amount1::try_from(confidential::Value::from(amount1))?, amount1);
        assert_eq!(Nonce::try_from(confidential::Nonce::from(nonce))?, nonce);
        assert_eq!(TokenAmount1::try_from(confidential::Value::from(tokens))?, tokens);
    }

    assert!(Asset1::try_from(confidential::Asset::Null).is_err());
    assert!(Amount1::try_from(confidential::Value::Null).is_err());
    assert!(Nonce::try_from(confidential::Nonce::Null).is_err());
    assert!(TokenAmount1::try_from(confidential::Value::Null).is_err());
    assert!(Ctx8::from_parts(vec![0; 64], 64, [0; 32]).is_err());
    assert!(Ctx8::from_parts(vec![0; 1], 0, [0; 32]).is_err());
    assert_eq!(Time(0).to_value(), Value::u32(0));
    Ok(())
}
