use std::collections::HashMap;
use std::fmt::{Debug, Formatter};
use std::marker::PhantomData;

use simplicityhl::num::U256;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::ValueConstructible;
use simplicityhl::{Arguments, ResolvedType, Value, WitnessValues};

use proptest::prelude::Rng;
use proptest::prelude::Strategy;
use proptest::strategy::{NewTree, ValueTree};
use proptest::test_runner::{TestRng, TestRunner};
use rand::prelude::IndexedRandom;

use smplx_sdk::program::{RandomArguments, RandomWitness};

static INTERESTING_U4: &[u8] = &[0, 1, 2, 7, 8, 14, 15];

static INTERESTING_U8: &[u8] = &[0, 1, 2, 16, 32, 64, 100, (1 << 7) - 1, 1 << 7, u8::MAX - 1, u8::MAX];

static INTERESTING_U16: &[u16] = &[
    0,
    1,
    2,
    16,
    32,
    64,
    100,
    1000,
    1024,
    4096,
    (1 << 7) - 1,
    1 << 7,
    u8::MAX as u16,
    1 << 8,
    (1 << 15) - 1,
    1 << 15,
    u16::MAX - 1,
    u16::MAX,
];

static INTERESTING_U32: &[u32] = &[
    0,
    1,
    2,
    16,
    32,
    64,
    100,
    1000,
    1024,
    4096,
    (1 << 7) - 1,
    1 << 7,
    u8::MAX as u32,
    1 << 8,
    (1 << 15) - 1,
    1 << 15,
    u16::MAX as u32,
    1 << 16,
    (1 << 31) - 1,
    1 << 31,
    u32::MAX - 1,
    u32::MAX,
];

static INTERESTING_U64: &[u64] = &[
    0,
    1,
    2,
    16,
    32,
    64,
    100,
    1000,
    1024,
    4096,
    (1 << 7) - 1,
    1 << 7,
    u8::MAX as u64,
    1 << 8,
    (1 << 15) - 1,
    1 << 15,
    u16::MAX as u64,
    1 << 16,
    (1 << 31) - 1,
    1 << 31,
    u32::MAX as u64,
    1 << 32,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX - 1,
    u64::MAX,
];

static INTERESTING_U128: &[u128] = &[
    0,
    1,
    2,
    16,
    32,
    64,
    100,
    1000,
    1024,
    4096,
    (1 << 7) - 1,
    1 << 7,
    u8::MAX as u128,
    1 << 8,
    (1 << 15) - 1,
    1 << 15,
    u16::MAX as u128,
    1 << 16,
    (1 << 31) - 1,
    1 << 31,
    u32::MAX as u128,
    1 << 32,
    (1 << 63) - 1,
    1 << 63,
    u64::MAX as u128,
    1 << 64,
    (1_u128 << 127) - 1,
    1_u128 << 127,
    u128::MAX - 1,
    u128::MAX,
];

pub struct InterestingRandom<Args, Wit> {
    phantom_data: PhantomData<(Args, Wit)>,
}

impl<Args, Wit> Default for InterestingRandom<Args, Wit> {
    fn default() -> Self {
        Self {
            phantom_data: PhantomData,
        }
    }
}

impl<T, E> Debug for InterestingRandom<T, E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "InterestingRandom trees...")
    }
}

pub struct InterestingRandomValueTree<T>(T);

impl<T: Clone + std::fmt::Debug> ValueTree for InterestingRandomValueTree<T> {
    type Value = T;
    fn current(&self) -> T {
        self.0.clone()
    }
    fn simplify(&mut self) -> bool {
        false
    }
    fn complicate(&mut self) -> bool {
        false
    }
}

impl<Args: RandomArguments + std::fmt::Debug, Wit: RandomWitness + std::fmt::Debug> Strategy
    for InterestingRandom<Args, Wit>
{
    type Tree = InterestingRandomValueTree<(Arguments, WitnessValues)>;
    type Value = (Arguments, WitnessValues);

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let args = Args::generate_arguments(runner.rng());
        let wit = Wit::generate_witness(runner.rng());

        let mut args_map = HashMap::new();
        for (name, val) in args.iter() {
            args_map.insert(
                name.clone(),
                generate_interesting_or_scratch_by_ty(val.ty(), runner.rng()),
            );
        }

        let mut wit_map = HashMap::new();
        for (name, val) in wit.iter() {
            wit_map.insert(
                name.clone(),
                generate_interesting_or_scratch_by_ty(val.ty(), runner.rng()),
            );
        }

        Ok(InterestingRandomValueTree((
            Arguments::from(args_map),
            WitnessValues::from(wit_map),
        )))
    }
}

pub fn generate_interesting_or_scratch_by_ty(ty: &ResolvedType, rng: &mut TestRng) -> Value {
    match ty.as_inner() {
        TypeInner::Either(left_ty, right_ty) => {
            let left_v: bool = rng.random();
            match left_v {
                true => Value::left(
                    generate_interesting_or_scratch_by_ty(left_ty, rng),
                    (**right_ty).clone(),
                ),
                false => Value::right(
                    (**left_ty).clone(),
                    generate_interesting_or_scratch_by_ty(right_ty, rng),
                ),
            }
        }
        TypeInner::Option(option_ty) => {
            let is_some: bool = rng.random();
            match is_some {
                true => Value::some(generate_interesting_or_scratch_by_ty(option_ty, rng)),
                false => Value::none((**option_ty).clone()),
            }
        }
        TypeInner::Boolean => Value::from(rng.random::<bool>()),
        TypeInner::UInt(x) => {
            let use_interesting: bool = rng.random();
            if use_interesting {
                match x {
                    UIntType::U1 => Value::u1(rng.random::<u8>() & 0x01),
                    UIntType::U2 => Value::u2(rng.random::<u8>() & 0x03),
                    UIntType::U4 => {
                        let val = *INTERESTING_U4.choose(rng).unwrap();
                        Value::u4(val)
                    }
                    UIntType::U8 => {
                        let val = *INTERESTING_U8.choose(rng).unwrap();
                        Value::u8(val)
                    }
                    UIntType::U16 => {
                        let val = *INTERESTING_U16.choose(rng).unwrap();
                        Value::u16(val)
                    }
                    UIntType::U32 => {
                        let val = *INTERESTING_U32.choose(rng).unwrap();
                        Value::u32(val)
                    }
                    UIntType::U64 => {
                        let val = *INTERESTING_U64.choose(rng).unwrap();
                        Value::u64(val)
                    }
                    UIntType::U128 => {
                        let val = *INTERESTING_U128.choose(rng).unwrap();
                        Value::u128(val)
                    }
                    UIntType::U256 => {
                        let val = *INTERESTING_U128.choose(rng).unwrap();
                        let mut bytes = [0u8; 32];
                        let val_bytes = val.to_be_bytes();
                        bytes[16..32].copy_from_slice(&val_bytes);
                        Value::u256(U256::from_byte_array(bytes))
                    }
                }
            } else {
                match x {
                    UIntType::U1 => Value::u1(rng.random::<u8>() & 0x0001),
                    UIntType::U2 => Value::u2(rng.random::<u8>() & 0x0003),
                    UIntType::U4 => Value::u4(rng.random::<u8>() & 0x000F),
                    UIntType::U8 => Value::u8(rng.random::<u8>()),
                    UIntType::U16 => Value::u16(rng.random::<u16>()),
                    UIntType::U32 => Value::u32(rng.random::<u32>()),
                    UIntType::U64 => Value::u64(rng.random::<u64>()),
                    UIntType::U128 => Value::u128(rng.random::<u128>()),
                    UIntType::U256 => Value::u256(U256::from_byte_array(rng.random())),
                }
            }
        }
        TypeInner::Tuple(tuple_ty) => {
            Value::tuple(tuple_ty.iter().map(|x| generate_interesting_or_scratch_by_ty(x, rng)))
        }
        TypeInner::Array(array_ty, size) => Value::array(
            (0..*size).map(|_| generate_interesting_or_scratch_by_ty(array_ty, rng)),
            (**array_ty).clone(),
        ),
        TypeInner::List(list_ty, size_pow_2) => {
            let size = rng.random_range(0..size_pow_2.get());
            Value::list(
                (0..size).map(|_| generate_interesting_or_scratch_by_ty(list_ty, rng)),
                (**list_ty).clone(),
                *size_pow_2,
            )
        }
        _ => Value::unit(),
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct InterestingU64Strategy;

impl Strategy for InterestingU64Strategy {
    type Tree = InterestingU64ValueTree;
    type Value = u64;

    fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
        let use_interesting: bool = runner.rng().random();
        let val = if use_interesting {
            *INTERESTING_U64.choose(runner.rng()).unwrap()
        } else {
            runner.rng().random::<u64>()
        };
        Ok(InterestingU64ValueTree { val })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct InterestingU64ValueTree {
    val: u64,
}

impl ValueTree for InterestingU64ValueTree {
    type Value = u64;

    fn current(&self) -> Self::Value {
        self.val
    }

    fn simplify(&mut self) -> bool {
        if self.val > 0 {
            self.val /= 2;
            true
        } else {
            false
        }
    }

    fn complicate(&mut self) -> bool {
        false
    }
}
