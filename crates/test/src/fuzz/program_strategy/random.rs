use simplicityhl::{Arguments, WitnessNameToValueMap, WitnessValues};

use proptest::test_runner::TestRng;

use rand::RngExt;

use crate::fuzz::program_strategy::{FuzzProgramSchema, FuzzProgramValues};
use crate::fuzz::utils::{generate_random_value_by_ty, random_fields};

/// A single randomly generated case.
pub(crate) struct RandomCase<'a> {
    schema: &'a FuzzProgramSchema,
    rng: &'a mut TestRng,
}

impl<'a> RandomCase<'a> {
    pub(crate) fn new(schema: &'a FuzzProgramSchema, rng: &'a mut TestRng) -> Self {
        Self { schema, rng }
    }

    pub(crate) fn generate(self) -> FuzzProgramValues {
        let witness = self
            .schema
            .witness_types
            .iter()
            .map(|(name, ty)| {
                let path = self.schema.spending_path(name);

                (
                    name.clone(),
                    path.draw(ty, &mut |ty| generate_random_value_by_ty(ty, self.rng)),
                )
            })
            .collect();

        FuzzProgramValues {
            arguments: Arguments::from_map(random_fields(&self.schema.argument_types, self.rng)),
            witness: WitnessValues::from_map(witness),
            storage: (0..self.schema.storage_capacity).map(|_| self.rng.random()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::test_runner::{Config, RngSeed, TestRunner};

    use simplicityhl::num::{NonZeroPow2Usize, U256};
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::{ResolvedType, TemplateProgramWitness, WitnessNameToValueMap};

    use smplx_sdk::program::ProgramSchema;

    use super::*;

    struct RichSchema;

    impl RichSchema {
        fn types(key: impl Fn(&'static str) -> TemplateProgramWitness) -> Vec<(TemplateProgramWitness, ResolvedType)> {
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

        fn runner() -> TestRunner {
            TestRunner::new(Config {
                rng_seed: RngSeed::Fixed(0x0000_0734),
                failure_persistence: None,
                ..Config::default()
            })
        }

        fn assert_matches(arguments: &Arguments, witness: &WitnessValues) {
            for (name, ty) in Self::argument_types() {
                assert!(arguments.get(&name).unwrap().is_of_type(&ty), "{name} is not {ty}");
            }

            for (name, ty) in Self::witness_types() {
                assert!(witness.get(&name).unwrap().is_of_type(&ty), "{name} is not {ty}");
            }
        }
    }

    impl ProgramSchema for RichSchema {
        type Arguments = ();
        type Witness = ();

        fn argument_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
            Self::types(TemplateProgramWitness::parameter_from_str)
        }

        fn witness_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
            Self::types(TemplateProgramWitness::witness_from_str)
        }

        fn literals() -> Vec<U256> {
            Vec::new()
        }
    }

    fn generate(runner: &mut TestRunner) -> FuzzProgramValues {
        RandomCase::new(&FuzzProgramSchema::of::<RichSchema>(), runner.rng()).generate()
    }

    #[test]
    fn strategy_is_persistent() {
        let mut runner_1 = RichSchema::runner();
        let mut runner_2 = RichSchema::runner();

        for _ in 0..1024 {
            let first = generate(&mut runner_1);
            let second = generate(&mut runner_2);

            assert_eq!(first, second, "same seed produced a different random case");
        }
    }

    #[test]
    fn values_match_their_declared_types() {
        let mut runner = RichSchema::runner();

        for _ in 0..1024 {
            let values = generate(&mut runner);

            RichSchema::assert_matches(&values.arguments, &values.witness);
        }
    }
}
