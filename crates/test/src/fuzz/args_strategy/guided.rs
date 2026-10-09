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

/// Big-endian 256-bit unsigned integer.
type Word = [u8; 32];

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
        let literals: Vec<Word> = P::literals().into_iter().map(U256::to_byte_array).collect();
        let argument_types = P::argument_types();
        let witness_types = P::witness_types();

        let rng = runner.rng();
        let mut leaves = Vec::new();

        let arguments = draw_fields(&argument_types, &literals, &mut leaves, rng);
        let witness = draw_fields(&witness_types, &literals, &mut leaves, rng);

        let mut words = reuse(&leaves, rng).into_iter();
        let mut next_word = |uint: UIntType| to_value(words.next().expect("one word per leaf"), uint);

        let arguments: HashMap<_, _> = arguments
            .into_iter()
            .map(|(name, value)| (name, map_uints(&value, &mut next_word)))
            .collect();
        let witness: HashMap<_, _> = witness
            .into_iter()
            .map(|(name, value)| (name, map_uints(&value, &mut next_word)))
            .collect();

        Ok(FixedValueTree((
            Arguments::from_map(arguments),
            WitnessValues::from_map(witness),
        )))
    }
}

fn draw_fields(
    types: &[(TemplateProgramWitness, ResolvedType)],
    literals: &[Word],
    leaves: &mut Vec<(usize, Word)>,
    rng: &mut TestRng,
) -> Vec<(TemplateProgramWitness, Value)> {
    types
        .iter()
        .map(|(name, ty)| (name.clone(), draw(ty, literals, leaves, rng)))
        .collect()
}

fn draw(ty: &ResolvedType, literals: &[Word], leaves: &mut Vec<(usize, Word)>, rng: &mut TestRng) -> Value {
    match ty.as_inner() {
        TypeInner::Boolean => Value::from(rng.random::<bool>()),
        TypeInner::UInt(uint) => {
            let bits = uint.bit_width().get();
            let word = draw_word(bits, literals, rng);

            leaves.push((bits, word));

            to_value(word, *uint)
        }
        TypeInner::Either(left, right) => {
            if rng.random() {
                Value::left(draw(left, literals, leaves, rng), (**right).clone())
            } else {
                Value::right((**left).clone(), draw(right, literals, leaves, rng))
            }
        }
        TypeInner::Option(inner) => {
            if rng.random() {
                Value::some(draw(inner, literals, leaves, rng))
            } else {
                Value::none((**inner).clone())
            }
        }
        TypeInner::Tuple(elements) => {
            let values: Vec<_> = elements.iter().map(|el| draw(el, literals, leaves, rng)).collect();

            Value::tuple(values)
        }
        TypeInner::Array(element, size) => {
            let values: Vec<_> = (0..*size).map(|_| draw(element, literals, leaves, rng)).collect();

            Value::array(values, (**element).clone())
        }
        TypeInner::List(element, bound) => {
            let max_len = bound.get() - 1;
            let len = if rng.random() {
                [0, 1, max_len][rng.random_range(0..3)].min(max_len)
            } else {
                rng.random_range(0..bound.get())
            };
            let values: Vec<_> = (0..len).map(|_| draw(element, literals, leaves, rng)).collect();

            Value::list(values, (**element).clone(), *bound)
        }
        _ => Value::unit(),
    }
}

fn draw_word(bits: usize, literals: &[Word], rng: &mut TestRng) -> Word {
    let fitting: Vec<Word> = literals.iter().copied().filter(|word| fits(word, bits)).collect();
    let literal_weight = if fitting.is_empty() { 0 } else { LITERAL_WEIGHT };

    let roll = rng.random_range(0..RANDOM_WEIGHT + BOUNDARY_WEIGHT + literal_weight);

    if roll < RANDOM_WEIGHT {
        let mut word: Word = rng.random();
        mask(&mut word, bits);

        word
    } else if roll < RANDOM_WEIGHT + BOUNDARY_WEIGHT {
        let boundaries = boundary_words(bits);

        boundaries[rng.random_range(0..boundaries.len())]
    } else {
        let literal = fitting[rng.random_range(0..fitting.len())];

        match rng.random_range(0..4) {
            0 => sub_one(literal).unwrap_or(literal),
            1 => add_one(literal).filter(|word| fits(word, bits)).unwrap_or(literal),
            _ => literal,
        }
    }
}

/// Final word of every leaf: either its own or, with `REUSE_PERCENT`, one of another leaf that fits.
fn reuse(leaves: &[(usize, Word)], rng: &mut TestRng) -> Vec<Word> {
    (0..leaves.len())
        .map(|index| {
            let (bits, word) = leaves[index];

            if rng.random_range(0..100) >= REUSE_PERCENT {
                return word;
            }

            let candidates: Vec<Word> = leaves
                .iter()
                .enumerate()
                .filter(|(other, (_, candidate))| *other != index && fits(candidate, bits))
                .map(|(_, (_, candidate))| *candidate)
                .collect();

            if candidates.is_empty() {
                word
            } else {
                candidates[rng.random_range(0..candidates.len())]
            }
        })
        .collect()
}

/// Rebuilds `value`, replacing every integer leaf in traversal order.
fn map_uints(value: &Value, next: &mut impl FnMut(UIntType) -> Value) -> Value {
    match (value.inner(), value.ty().as_inner()) {
        (ValueInner::UInt(_), TypeInner::UInt(uint)) => next(*uint),
        (ValueInner::Either(either), TypeInner::Either(left_ty, right_ty)) => match either {
            Either::Left(left) => Value::left(map_uints(left, next), (**right_ty).clone()),
            Either::Right(right) => Value::right((**left_ty).clone(), map_uints(right, next)),
        },
        (ValueInner::Option(Some(inner)), _) => Value::some(map_uints(inner, next)),
        (ValueInner::Tuple(elements), _) => {
            let values: Vec<_> = elements.iter().map(|el| map_uints(el, next)).collect();

            Value::tuple(values)
        }
        (ValueInner::Array(elements), TypeInner::Array(element_ty, _)) => {
            let values: Vec<_> = elements.iter().map(|el| map_uints(el, next)).collect();

            Value::array(values, (**element_ty).clone())
        }
        (ValueInner::List(elements, bound), TypeInner::List(element_ty, _)) => {
            let values: Vec<_> = elements.iter().map(|el| map_uints(el, next)).collect();

            Value::list(values, (**element_ty).clone(), *bound)
        }
        _ => value.clone(),
    }
}

fn to_value(word: Word, uint: UIntType) -> Value {
    let low = u128::from_be_bytes(word[16..].try_into().expect("16 bytes"));

    match uint {
        UIntType::U1 => Value::u1(low as u8),
        UIntType::U2 => Value::u2(low as u8),
        UIntType::U4 => Value::u4(low as u8),
        UIntType::U8 => Value::u8(low as u8),
        UIntType::U16 => Value::u16(low as u16),
        UIntType::U32 => Value::u32(low as u32),
        UIntType::U64 => Value::u64(low as u64),
        UIntType::U128 => Value::u128(low),
        UIntType::U256 => Value::u256(U256::from_byte_array(word)),
    }
}

fn boundary_words(bits: usize) -> Vec<Word> {
    let mut words: Vec<Word> = BOUNDARY_SMALL.iter().map(|small| from_u128(*small)).collect();

    for power in BOUNDARY_POWERS.iter().copied().filter(|power| *power < bits) {
        let word = pow2(power);

        words.push(word);
        words.extend(sub_one(word));
    }

    let max = max_word(bits);

    words.push(max);
    words.extend(sub_one(max));

    words.retain(|word| fits(word, bits));
    words.sort_unstable();
    words.dedup();

    words
}

fn from_u128(value: u128) -> Word {
    let mut word = [0; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());

    word
}

fn pow2(power: usize) -> Word {
    let mut word = [0; 32];
    word[31 - power / 8] = 1 << (power % 8);

    word
}

fn max_word(bits: usize) -> Word {
    let mut word = [0xff; 32];
    mask(&mut word, bits);

    word
}

/// Clears every bit at or above `bits`.
fn mask(word: &mut Word, bits: usize) {
    for (index, byte) in word.iter_mut().rev().enumerate() {
        let low = index * 8;

        if low >= bits {
            *byte = 0;
        } else if bits - low < 8 {
            *byte &= (1 << (bits - low)) - 1;
        }
    }
}

fn fits(word: &Word, bits: usize) -> bool {
    *word <= max_word(bits)
}

fn add_one(mut word: Word) -> Option<Word> {
    for byte in word.iter_mut().rev() {
        let (sum, overflow) = byte.overflowing_add(1);
        *byte = sum;

        if !overflow {
            return Some(word);
        }
    }

    None
}

fn sub_one(mut word: Word) -> Option<Word> {
    for byte in word.iter_mut().rev() {
        let (difference, underflow) = byte.overflowing_sub(1);
        *byte = difference;

        if !underflow {
            return Some(word);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use proptest::prelude::Strategy;
    use proptest::strategy::ValueTree;

    use simplicityhl::num::U256;
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{ResolvedType, TemplateProgramWitness, Value, WitnessNameToValueMap};

    use smplx_sdk::program::ProgramSchema;

    use crate::fuzz::args_strategy::test_schema::{RichSchema, assert_matches_schema, deterministic_runner};

    use super::*;

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

        let mut runner_1 = deterministic_runner();
        let mut runner_2 = deterministic_runner();

        for _ in 0..1024 {
            let first = strategy.new_tree(&mut runner_1).unwrap().current();
            let second = strategy.new_tree(&mut runner_2).unwrap().current();

            assert_eq!(first, second, "same seed produced a different guided case");
        }
    }

    #[test]
    fn values_match_their_declared_types() {
        let strategy = Guided::<RichSchema>::default();
        let mut runner = deterministic_runner();

        for _ in 0..1024 {
            let (arguments, witness) = strategy.new_tree(&mut runner).unwrap().current();

            assert_matches_schema::<RichSchema>(&arguments, &witness);
        }
    }

    #[test]
    fn literals_are_reached_quickly() {
        let strategy = Guided::<MagicSchema>::default();
        let mut runner = deterministic_runner();
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
        let (small, big, other) = (from_u128(7), from_u128(1 << 40), from_u128(1 << 41));
        let leaves = [(8, small), (64, big), (64, other)];
        let mut runner = deterministic_runner();
        let mut copied = false;

        for _ in 0..1024 {
            let words = reuse(&leaves, runner.rng());

            assert_eq!(words[0], small);
            assert!([big, other, small].contains(&words[1]));
            assert!([big, other, small].contains(&words[2]));

            copied |= words[1] != big || words[2] != other;
        }

        assert!(copied);
    }

    #[test]
    fn boundaries_cover_the_full_width() {
        let max = [0xff; 32];

        assert!(boundary_words(256).contains(&max));
        assert!(boundary_words(256).contains(&pow2(255)));
        assert_eq!(boundary_words(1), vec![from_u128(0), from_u128(1)]);
        assert!(boundary_words(8).iter().all(|word| fits(word, 8)));
    }

    #[test]
    fn word_arithmetic() {
        assert_eq!(add_one(from_u128(255)), Some(from_u128(256)));
        assert_eq!(sub_one(from_u128(256)), Some(from_u128(255)));
        assert_eq!(add_one([0xff; 32]), None);
        assert_eq!(sub_one(from_u128(0)), None);
        assert!(fits(&from_u128(15), 4));
        assert!(!fits(&from_u128(16), 4));
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
        let mut next = |uint: UIntType| to_value(from_u128(3), uint);

        let mapped = map_uints(&value, &mut next);

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
