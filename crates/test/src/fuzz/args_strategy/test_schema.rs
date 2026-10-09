use proptest::test_runner::{Config, RngSeed, TestRunner};

use simplicityhl::num::{NonZeroPow2Usize, U256};
use simplicityhl::types::TypeConstructible;
use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, WitnessNameToValueMap, WitnessValues};

use smplx_sdk::program::ProgramSchema;

pub(crate) struct RichSchema;

fn rich_types(key: impl Fn(&'static str) -> TemplateProgramWitness) -> Vec<(TemplateProgramWitness, ResolvedType)> {
    vec![
        (key("U1"), ResolvedType::u1()),
        (key("U2"), ResolvedType::u2()),
        (key("U4"), ResolvedType::u4()),
        (key("U8"), ResolvedType::u8()),
        (key("U16"), ResolvedType::u16()),
        (key("U32"), ResolvedType::u32()),
        (key("U64"), ResolvedType::u64()),
        (key("U128"), ResolvedType::u128()),
        (key("U256"), ResolvedType::u256()),
        (key("BOOL"), ResolvedType::boolean()),
        (
            key("EITHER"),
            ResolvedType::either(ResolvedType::u8(), ResolvedType::u16()),
        ),
        (key("OPTION"), ResolvedType::option(ResolvedType::u32())),
        (
            key("TUPLE"),
            ResolvedType::tuple([ResolvedType::u8(), ResolvedType::u256(), ResolvedType::boolean()]),
        ),
        (key("ARRAY"), ResolvedType::array(ResolvedType::u16(), 3)),
        (
            key("LIST"),
            ResolvedType::list(ResolvedType::u64(), NonZeroPow2Usize::new(8).unwrap()),
        ),
    ]
}

impl ProgramSchema for RichSchema {
    fn argument_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
        rich_types(TemplateProgramWitness::parameter_from_str)
    }

    fn witness_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
        rich_types(TemplateProgramWitness::witness_from_str)
    }

    fn literals() -> Vec<U256> {
        vec![
            U256::from(1337_u16),
            U256::from(u128::MAX),
            U256::from_byte_array([0xff; 32]),
        ]
    }
}

pub(crate) fn deterministic_runner() -> TestRunner {
    TestRunner::new(Config {
        rng_seed: RngSeed::Fixed(0x0000_0734),
        failure_persistence: None,
        ..Config::default()
    })
}

pub(crate) fn assert_matches_schema<P: ProgramSchema>(arguments: &Arguments, witness: &WitnessValues) {
    for (name, ty) in P::argument_types() {
        assert!(arguments.get(&name).unwrap().is_of_type(&ty), "{name} is not {ty}");
    }

    for (name, ty) in P::witness_types() {
        assert!(witness.get(&name).unwrap().is_of_type(&ty), "{name} is not {ty}");
    }
}
