use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

use simplicityhl::either::Either;
use simplicityhl::types::TypeInner;
use simplicityhl::value::{ValueConstructible, ValueInner};
use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::Strategy;
use proptest::strategy::NewTree;
use proptest::test_runner::{TestRng, TestRunner};

use rand::RngExt;

use smplx_sdk::program::ProgramSchema;

use crate::fuzz::common::FixedValueTree;
use crate::fuzz::program_strategy::FuzzProgramSchema;
use crate::fuzz::program_strategy::{FuzzProgramValues, Word};
use crate::fuzz::scenario_strategy::FuzzTransactionSchema;
use crate::fuzz::witness_path::WitnessPath;

// TODO: make the weights configurable.
const RANDOM_WEIGHT: u32 = 40;
const BOUNDARY_WEIGHT: u32 = 20;
const LITERAL_WEIGHT: u32 = 20;
const UTXO_WEIGHT: u32 = 40;
const REUSE_PERCENT: u32 = 20;

/// Draws each integer from uniform random, boundary or literal values, then copies values between
/// integers of the same case.
pub struct Guided<P> {
    storage_capacity: usize,
    _program: PhantomData<fn() -> P>,
}

impl<P> Guided<P> {
    #[must_use]
    pub fn with_storage_capacity(mut self, storage_capacity: usize) -> Self {
        self.storage_capacity = storage_capacity;

        self
    }
}

impl<P> Default for Guided<P> {
    fn default() -> Self {
        Self {
            storage_capacity: 0,
            _program: PhantomData,
        }
    }
}

impl<P> Debug for Guided<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        f.debug_struct("Guided")
            .field("storage_capacity", &self.storage_capacity)
            .finish()
    }
}

impl<P: ProgramSchema> Strategy for Guided<P> {
    type Tree = FixedValueTree<FuzzProgramValues>;
    type Value = FuzzProgramValues;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let schema = FuzzProgramSchema::of::<P>().with_storage_capacity(self.storage_capacity);
        let values = GuidedCase::new(&[&schema], runner.rng()).generate();

        Ok(FixedValueTree(values.into_iter().next().expect("one schema")))
    }
}

/// State of a single generated case. Programs generated together share literals and values.
pub(crate) struct GuidedCase<'a> {
    schemas: &'a [&'a FuzzProgramSchema],
    literals: Vec<Word>,
    utxos_max: u32,
    leaves: Vec<(usize, Word)>,
    rng: &'a mut TestRng,
}

impl<'a> GuidedCase<'a> {
    pub(crate) fn new(schemas: &'a [&'a FuzzProgramSchema], rng: &'a mut TestRng) -> Self {
        Self {
            schemas,
            literals: schemas
                .iter()
                .flat_map(|schema| schema.literals.iter().copied().map(Word::from))
                .collect(),
            utxos_max: 0,
            leaves: Vec::new(),
            rng,
        }
    }

    /// Draws from the transaction's values too: its literals join the pool and indices up to `utxos_max` come up often.
    #[must_use]
    pub(crate) fn with_transaction_schema(mut self, schema: &FuzzTransactionSchema) -> Self {
        self.literals.extend(schema.literals.iter().copied().map(Word::from));
        self.utxos_max = schema.utxos_max;

        self
    }

    /// The values of each schema, in order. Storage slots are drawn like `u256` leaves.
    pub(crate) fn generate(mut self) -> Vec<FuzzProgramValues> {
        let schemas = self.schemas;
        let fields: Vec<_> = schemas
            .iter()
            .map(|schema| {
                let arguments = self.draw_fields(&schema.argument_types, |_| WitnessPath::empty());
                let witness = self.draw_fields(&schema.witness_types, |field| schema.spending_path(field));

                for _ in 0..schema.storage_capacity {
                    let word = self.draw_word(256);
                    self.leaves.push((256, word));
                }

                (arguments, witness, schema.storage_capacity)
            })
            .collect();

        let mut words = self.reuse().into_iter();

        fields
            .into_iter()
            .map(|(arguments, witness, storage_capacity)| {
                let arguments: HashMap<_, _> = arguments
                    .into_iter()
                    .map(|(name, value)| (name, Self::map_uints(&value, &mut words)))
                    .collect();
                let witness: HashMap<_, _> = witness
                    .into_iter()
                    .map(|(name, value)| (name, Self::map_uints(&value, &mut words)))
                    .collect();

                FuzzProgramValues {
                    arguments: Arguments::from_map(arguments),
                    witness: WitnessValues::from_map(witness),
                    storage: (0..storage_capacity)
                        .map(|_| words.next().expect("one word per leaf").to_byte_array())
                        .collect(),
                }
            })
            .collect()
    }

    /// Each field takes the branches of the given path.
    fn draw_fields<'p>(
        &mut self,
        types: &[(TemplateProgramWitness, ResolvedType)],
        path_of: impl Fn(&TemplateProgramWitness) -> &'p WitnessPath,
    ) -> Vec<(TemplateProgramWitness, Value)> {
        types
            .iter()
            .map(|(name, ty)| (name.clone(), path_of(name).draw(ty, &mut |ty| self.draw(ty))))
            .collect()
    }

    fn draw(&mut self, ty: &ResolvedType) -> Value {
        match ty.as_inner() {
            TypeInner::Boolean => Value::from(self.rng.random::<bool>()),
            TypeInner::UInt(uint) => {
                let bits = uint.bit_width().get();
                let word = self.draw_word(bits);

                self.leaves.push((bits, word));

                word.to_value(*uint)
            }
            TypeInner::Either(left, right) => {
                if self.rng.random() {
                    Value::left(self.draw(left), (**right).clone())
                } else {
                    Value::right((**left).clone(), self.draw(right))
                }
            }
            TypeInner::Option(inner) => {
                if self.rng.random() {
                    Value::some(self.draw(inner))
                } else {
                    Value::none((**inner).clone())
                }
            }
            TypeInner::Tuple(elements) => {
                let values: Vec<_> = elements.iter().map(|el| self.draw(el)).collect();

                Value::tuple(values)
            }
            TypeInner::Array(element, size) => {
                let values: Vec<_> = (0..*size).map(|_| self.draw(element)).collect();

                Value::array(values, (**element).clone())
            }
            TypeInner::List(element, bound) => {
                let max_len = bound.get() - 1;
                let len = if self.rng.random() {
                    [0, 1, max_len][self.rng.random_range(0..3)].min(max_len)
                } else {
                    self.rng.random_range(0..bound.get())
                };
                let values: Vec<_> = (0..len).map(|_| self.draw(element)).collect();

                Value::list(values, (**element).clone(), *bound)
            }
            _ => Value::unit(),
        }
    }

    fn draw_word(&mut self, bits: usize) -> Word {
        let fitting: Vec<Word> = self.literals.iter().copied().filter(|word| word.fits(bits)).collect();
        let literal_weight = if fitting.is_empty() { 0 } else { LITERAL_WEIGHT };
        let utxo_weight = if self.utxos_max > 0 && Word::from_u128(self.utxos_max.into()).fits(bits) {
            UTXO_WEIGHT
        } else {
            0
        };

        let roll = self
            .rng
            .random_range(0..RANDOM_WEIGHT + BOUNDARY_WEIGHT + literal_weight + utxo_weight);

        if roll >= RANDOM_WEIGHT + BOUNDARY_WEIGHT + literal_weight {
            Word::from_u128(self.rng.random_range(0..=self.utxos_max).into())
        } else if roll < RANDOM_WEIGHT {
            Word::random(bits, self.rng)
        } else if roll < RANDOM_WEIGHT + BOUNDARY_WEIGHT {
            let boundaries = Word::boundaries(bits);

            boundaries[self.rng.random_range(0..boundaries.len())]
        } else {
            let literal = fitting[self.rng.random_range(0..fitting.len())];

            match self.rng.random_range(0..4) {
                0 => literal.sub_one().unwrap_or(literal),
                1 => literal.add_one().filter(|word| word.fits(bits)).unwrap_or(literal),
                _ => literal,
            }
        }
    }

    /// Final word of every leaf: either its own or, with `REUSE_PERCENT`, one of another leaf that fits.
    fn reuse(&mut self) -> Vec<Word> {
        (0..self.leaves.len())
            .map(|index| {
                let (bits, word) = self.leaves[index];

                if self.rng.random_range(0..100) >= REUSE_PERCENT {
                    return word;
                }

                let candidates: Vec<Word> = self
                    .leaves
                    .iter()
                    .enumerate()
                    .filter(|(other, (_, candidate))| *other != index && candidate.fits(bits))
                    .map(|(_, (_, candidate))| *candidate)
                    .collect();

                if candidates.is_empty() {
                    word
                } else {
                    candidates[self.rng.random_range(0..candidates.len())]
                }
            })
            .collect()
    }

    /// Rebuilds `value`, replacing every integer leaf in traversal order.
    fn map_uints(value: &Value, words: &mut impl Iterator<Item = Word>) -> Value {
        match (value.inner(), value.ty().as_inner()) {
            (ValueInner::UInt(_), TypeInner::UInt(uint)) => words.next().expect("one word per leaf").to_value(*uint),
            (ValueInner::Either(either), TypeInner::Either(left_ty, right_ty)) => match either {
                Either::Left(left) => Value::left(Self::map_uints(left, words), (**right_ty).clone()),
                Either::Right(right) => Value::right((**left_ty).clone(), Self::map_uints(right, words)),
            },
            (ValueInner::Option(Some(inner)), _) => Value::some(Self::map_uints(inner, words)),
            (ValueInner::Tuple(elements), _) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, words)).collect();

                Value::tuple(values)
            }
            (ValueInner::Array(elements), TypeInner::Array(element_ty, _)) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, words)).collect();

                Value::array(values, (**element_ty).clone())
            }
            (ValueInner::List(elements, bound), TypeInner::List(element_ty, _)) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, words)).collect();

                Value::list(values, (**element_ty).clone(), *bound)
            }
            _ => value.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::Strategy;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngSeed};

    use simplicityhl::num::{NonZeroPow2Usize, U256};
    use simplicityhl::types::TypeConstructible;

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
            vec![
                U256::from(1337_u16),
                U256::from(u128::MAX),
                U256::from_byte_array([0xff; 32]),
            ]
        }
    }

    struct MagicSchema;

    impl ProgramSchema for MagicSchema {
        type Arguments = ();
        type Witness = ();

        fn argument_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
            Vec::new()
        }

        fn witness_types() -> Vec<(TemplateProgramWitness, ResolvedType)> {
            vec![(TemplateProgramWitness::witness_from_str("VALUE"), ResolvedType::u16())]
        }

        fn literals() -> Vec<U256> {
            vec![U256::from(1337_u16)]
        }
    }

    #[test]
    fn strategy_is_persistent() {
        let strategy = Guided::<RichSchema>::default();

        let mut runner_1 = RichSchema::runner();
        let mut runner_2 = RichSchema::runner();

        for _ in 0..1024 {
            let first = strategy.new_tree(&mut runner_1).unwrap().current();
            let second = strategy.new_tree(&mut runner_2).unwrap().current();

            assert_eq!(first, second, "same seed produced a different guided case");
        }
    }

    #[test]
    fn values_match_their_declared_types() {
        let strategy = Guided::<RichSchema>::default();
        let mut runner = RichSchema::runner();

        for _ in 0..1024 {
            let values = strategy.new_tree(&mut runner).unwrap().current();

            RichSchema::assert_matches(&values.arguments, &values.witness);
        }
    }

    #[test]
    fn literals_are_reached_quickly() {
        let strategy = Guided::<MagicSchema>::default();
        let mut runner = RichSchema::runner();
        let magic = Value::u16(1337);
        let name = TemplateProgramWitness::witness_from_str("VALUE");

        let found = (0..100).any(|_| {
            let values = strategy.new_tree(&mut runner).unwrap().current();

            values.witness.get(&name) == Some(&magic)
        });

        assert!(found);
    }

    #[test]
    fn reuse_only_copies_other_leaves_that_fit() {
        let (small, big, other) = (Word::from_u128(7), Word::from_u128(1 << 40), Word::from_u128(1 << 41));
        let leaves = vec![(8, small), (64, big), (64, other)];
        let mut runner = RichSchema::runner();
        let mut copied = false;

        for _ in 0..1024 {
            let words = GuidedCase {
                schemas: &[],
                literals: Vec::new(),
                utxos_max: 0,
                leaves: leaves.clone(),
                rng: runner.rng(),
            }
            .reuse();

            assert_eq!(words[0], small);
            assert!([big, other, small].contains(&words[1]));
            assert!([big, other, small].contains(&words[2]));

            copied |= words[1] != big || words[2] != other;
        }

        assert!(copied);
    }

    #[test]
    fn map_uints_preserves_structure() {
        let ty = ResolvedType::tuple([
            ResolvedType::either(ResolvedType::u8(), ResolvedType::boolean()),
            ResolvedType::option(ResolvedType::u16()),
        ]);
        let value = Value::tuple([
            Value::left(Value::u8(1), ResolvedType::boolean()),
            Value::some(Value::u16(2)),
        ]);
        let mapped = GuidedCase::map_uints(&value, &mut std::iter::repeat(Word::from_u128(3)));

        assert!(mapped.is_of_type(&ty));
        assert_eq!(
            mapped,
            Value::tuple([
                Value::left(Value::u8(3), ResolvedType::boolean()),
                Value::some(Value::u16(3)),
            ])
        );
    }
}
