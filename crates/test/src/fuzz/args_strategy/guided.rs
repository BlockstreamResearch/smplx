use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result};
use std::marker::PhantomData;

use simplicityhl::either::Either;
use simplicityhl::num::U256;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::{ValueConstructible, ValueInner};
use simplicityhl::{Arguments, ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};

use proptest::prelude::{Rng, Strategy};
use proptest::strategy::NewTree;
use proptest::test_runner::{TestRng, TestRunner};

use smplx_sdk::program::ProgramSchema;

use crate::fuzz::args_strategy::FixedValueTree;

// TODO: make the weights configurable.
const RANDOM_WEIGHT: u32 = 40;
const BOUNDARY_WEIGHT: u32 = 20;
const LITERAL_WEIGHT: u32 = 20;
const REUSE_PERCENT: u32 = 20;

const BOUNDARY_POWERS: &[usize] = &[4, 5, 6, 7, 8, 10, 12, 15, 16, 31, 32, 63, 64, 127, 128, 255];
const BOUNDARY_SMALL: &[u128] = &[0, 1, 2, 100, 1000];

/// Draws each integer from uniform random, boundary or literal values, then copies values between
/// integers of the same case.
pub struct Guided<P> {
    _program: PhantomData<fn() -> P>,
}

impl<P> Default for Guided<P> {
    fn default() -> Self {
        Self { _program: PhantomData }
    }
}

impl<P> Debug for Guided<P> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        writeln!(f, "Guided trees...")
    }
}

impl<P: ProgramSchema> Strategy for Guided<P> {
    type Tree = FixedValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let case = GuidedCase::new(&P::literals(), runner.rng()).generate(&P::argument_types(), &P::witness_types());

        Ok(FixedValueTree(case))
    }
}

/// State of a single generated case.
struct GuidedCase<'a> {
    literals: Vec<Word>,
    leaves: Vec<(usize, Word)>,
    rng: &'a mut TestRng,
}

impl<'a> GuidedCase<'a> {
    fn new(literals: &[U256], rng: &'a mut TestRng) -> Self {
        Self {
            literals: literals.iter().copied().map(Word::from).collect(),
            leaves: Vec::new(),
            rng,
        }
    }

    fn generate(
        mut self,
        argument_types: &[(TemplateProgramWitness, ResolvedType)],
        witness_types: &[(TemplateProgramWitness, ResolvedType)],
    ) -> (Arguments, WitnessValues) {
        let arguments = self.draw_fields(argument_types);
        let witness = self.draw_fields(witness_types);

        let mut words = self.reuse().into_iter();
        let mut next_word = |uint: UIntType| words.next().expect("one word per leaf").to_value(uint);

        let arguments: HashMap<_, _> = arguments
            .into_iter()
            .map(|(name, value)| (name, Self::map_uints(&value, &mut next_word)))
            .collect();
        let witness: HashMap<_, _> = witness
            .into_iter()
            .map(|(name, value)| (name, Self::map_uints(&value, &mut next_word)))
            .collect();

        (Arguments::from_map(arguments), WitnessValues::from_map(witness))
    }

    fn draw_fields(
        &mut self,
        types: &[(TemplateProgramWitness, ResolvedType)],
    ) -> Vec<(TemplateProgramWitness, Value)> {
        types.iter().map(|(name, ty)| (name.clone(), self.draw(ty))).collect()
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

        let roll = self
            .rng
            .random_range(0..RANDOM_WEIGHT + BOUNDARY_WEIGHT + literal_weight);

        if roll < RANDOM_WEIGHT {
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
    fn map_uints(value: &Value, next: &mut impl FnMut(UIntType) -> Value) -> Value {
        match (value.inner(), value.ty().as_inner()) {
            (ValueInner::UInt(_), TypeInner::UInt(uint)) => next(*uint),
            (ValueInner::Either(either), TypeInner::Either(left_ty, right_ty)) => match either {
                Either::Left(left) => Value::left(Self::map_uints(left, next), (**right_ty).clone()),
                Either::Right(right) => Value::right((**left_ty).clone(), Self::map_uints(right, next)),
            },
            (ValueInner::Option(Some(inner)), _) => Value::some(Self::map_uints(inner, next)),
            (ValueInner::Tuple(elements), _) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, next)).collect();

                Value::tuple(values)
            }
            (ValueInner::Array(elements), TypeInner::Array(element_ty, _)) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, next)).collect();

                Value::array(values, (**element_ty).clone())
            }
            (ValueInner::List(elements, bound), TypeInner::List(element_ty, _)) => {
                let values: Vec<_> = elements.iter().map(|el| Self::map_uints(el, next)).collect();

                Value::list(values, (**element_ty).clone(), *bound)
            }
            _ => value.clone(),
        }
    }
}

/// Big-endian 256-bit unsigned integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Word([u8; 32]);

impl From<U256> for Word {
    fn from(value: U256) -> Self {
        Self(value.to_byte_array())
    }
}

impl Word {
    fn from_u128(value: u128) -> Self {
        let mut bytes = [0; 32];
        bytes[16..].copy_from_slice(&value.to_be_bytes());

        Self(bytes)
    }

    fn pow2(power: usize) -> Self {
        let mut bytes = [0; 32];
        bytes[31 - power / 8] = 1 << (power % 8);

        Self(bytes)
    }

    fn max(bits: usize) -> Self {
        Self([0xff; 32]).masked(bits)
    }

    fn random(bits: usize, rng: &mut TestRng) -> Self {
        Self(rng.random()).masked(bits)
    }

    fn boundaries(bits: usize) -> Vec<Self> {
        let mut words: Vec<Self> = BOUNDARY_SMALL.iter().map(|small| Self::from_u128(*small)).collect();

        for power in BOUNDARY_POWERS.iter().copied().filter(|power| *power < bits) {
            let word = Self::pow2(power);

            words.push(word);
            words.extend(word.sub_one());
        }

        let max = Self::max(bits);

        words.push(max);
        words.extend(max.sub_one());

        words.retain(|word| word.fits(bits));
        words.sort_unstable();
        words.dedup();

        words
    }

    /// Clears every bit at or above `bits`.
    fn masked(mut self, bits: usize) -> Self {
        for (index, byte) in self.0.iter_mut().rev().enumerate() {
            let low = index * 8;

            if low >= bits {
                *byte = 0;
            } else if bits - low < 8 {
                *byte &= (1 << (bits - low)) - 1;
            }
        }

        self
    }

    fn fits(&self, bits: usize) -> bool {
        *self <= Self::max(bits)
    }

    fn add_one(mut self) -> Option<Self> {
        for byte in self.0.iter_mut().rev() {
            let (sum, overflow) = byte.overflowing_add(1);
            *byte = sum;

            if !overflow {
                return Some(self);
            }
        }

        None
    }

    fn sub_one(mut self) -> Option<Self> {
        for byte in self.0.iter_mut().rev() {
            let (difference, underflow) = byte.overflowing_sub(1);
            *byte = difference;

            if !underflow {
                return Some(self);
            }
        }

        None
    }

    fn to_value(self, uint: UIntType) -> Value {
        let low = u128::from_be_bytes(self.0[16..].try_into().expect("16 bytes"));

        match uint {
            UIntType::U1 => Value::u1(low as u8),
            UIntType::U2 => Value::u2(low as u8),
            UIntType::U4 => Value::u4(low as u8),
            UIntType::U8 => Value::u8(low as u8),
            UIntType::U16 => Value::u16(low as u16),
            UIntType::U32 => Value::u32(low as u32),
            UIntType::U64 => Value::u64(low as u64),
            UIntType::U128 => Value::u128(low),
            UIntType::U256 => Value::u256(U256::from_byte_array(self.0)),
        }
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::Strategy;
    use proptest::strategy::ValueTree;
    use proptest::test_runner::{Config, RngSeed};

    use simplicityhl::num::NonZeroPow2Usize;
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
            let (arguments, witness) = strategy.new_tree(&mut runner).unwrap().current();

            RichSchema::assert_matches(&arguments, &witness);
        }
    }

    #[test]
    fn literals_are_reached_quickly() {
        let strategy = Guided::<MagicSchema>::default();
        let mut runner = RichSchema::runner();
        let magic = Value::u16(1337);
        let name = TemplateProgramWitness::witness_from_str("VALUE");

        let found = (0..100).any(|_| {
            let (_, witness) = strategy.new_tree(&mut runner).unwrap().current();

            witness.get(&name) == Some(&magic)
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
                literals: Vec::new(),
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
    fn boundaries_cover_the_full_width() {
        let max = Word([0xff; 32]);

        assert!(Word::boundaries(256).contains(&max));
        assert!(Word::boundaries(256).contains(&Word::pow2(255)));
        assert_eq!(Word::boundaries(1), vec![Word::from_u128(0), Word::from_u128(1)]);
        assert!(Word::boundaries(8).iter().all(|word| word.fits(8)));
    }

    #[test]
    fn word_arithmetic() {
        assert_eq!(Word::from_u128(255).add_one(), Some(Word::from_u128(256)));
        assert_eq!(Word::from_u128(256).sub_one(), Some(Word::from_u128(255)));
        assert_eq!(Word([0xff; 32]).add_one(), None);
        assert_eq!(Word::from_u128(0).sub_one(), None);
        assert!(Word::from_u128(15).fits(4));
        assert!(!Word::from_u128(16).fits(4));
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
        let mut next = |uint: UIntType| Word::from_u128(3).to_value(uint);

        let mapped = GuidedCase::map_uints(&value, &mut next);

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
