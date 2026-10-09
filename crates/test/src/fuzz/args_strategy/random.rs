use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::Strategy;
use proptest::strategy::NewTree;
use proptest::test_runner::TestRunner;

use smplx_sdk::program::ProgramSchema;

use crate::fuzz::args_strategy::FixedValueTree;
use crate::fuzz::utils::{random_arguments, random_witness};

pub struct Random<P> {
    _program: PhantomData<fn() -> P>,
}

impl<P> Default for Random<P> {
    fn default() -> Self {
        Self { _program: PhantomData }
    }
}

impl<P> Debug for Random<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Random trees...")
    }
}

impl<P: ProgramSchema> Strategy for Random<P> {
    type Tree = FixedValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let arguments = random_arguments::<P, _>(runner.rng());
        let witness = random_witness::<P, _>(runner.rng());

        Ok(FixedValueTree((arguments, witness)))
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::Strategy;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngSeed, TestRunner};

    use simplicityhl::num::{NonZeroPow2Usize, U256};
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::{ResolvedType, TemplateProgramWitness, WitnessNameToValueMap};

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

    #[test]
    fn strategy_is_persistent() {
        let strategy = Random::<RichSchema>::default();

        let mut runner_1 = RichSchema::runner();
        let mut runner_2 = RichSchema::runner();

        for _ in 0..1024 {
            let first = strategy.new_tree(&mut runner_1).unwrap().current();
            let second = strategy.new_tree(&mut runner_2).unwrap().current();

            assert_eq!(first, second, "same seed produced a different random case");
        }
    }

    #[test]
    fn values_match_their_declared_types() {
        let strategy = Random::<RichSchema>::default();
        let mut runner = RichSchema::runner();

        for _ in 0..1024 {
            let (arguments, witness) = strategy.new_tree(&mut runner).unwrap().current();

            RichSchema::assert_matches(&arguments, &witness);
        }
    }
}
