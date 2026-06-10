use rand::Rng;
use simplicityhl::num::{NonZeroPow2Usize, U256};
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::ValueConstructible;
use simplicityhl::{ResolvedType, Value};

pub fn generate_value_by_ty<R: Rng + ?Sized>(ty: &ResolvedType, rng: &mut R) -> Value {
    match ty.as_inner() {
        TypeInner::Either(left_ty, right_ty) => match rng.random::<bool>() {
            true => Value::left(generate_value_by_ty(left_ty, rng), (**right_ty).clone()),
            false => Value::right((**left_ty).clone(), generate_value_by_ty(right_ty, rng)),
        },
        TypeInner::Option(option_ty) => match rng.random::<bool>() {
            true => Value::some(generate_value_by_ty(option_ty, rng)),
            false => Value::none((**option_ty).clone()),
        },
        TypeInner::Boolean => Value::from(rng.random::<bool>()),
        TypeInner::UInt(x) => match x {
            UIntType::U1 => Value::u1(rng.random::<u8>() & 0x0001),
            UIntType::U2 => Value::u2(rng.random::<u8>() & 0x0003),
            UIntType::U4 => Value::u4(rng.random::<u8>() & 0x000F),
            UIntType::U8 => Value::u8(rng.random::<u8>()),
            UIntType::U16 => Value::u16(rng.random::<u16>()),
            UIntType::U32 => Value::u32(rng.random::<u32>()),
            UIntType::U64 => Value::u64(rng.random::<u64>()),
            UIntType::U128 => Value::u128(rng.random::<u128>()),
            UIntType::U256 => Value::u256(U256::from_byte_array(rng.random())),
        },
        TypeInner::Tuple(tuple_ty) => Value::tuple(tuple_ty.iter().map(|x| generate_value_by_ty(x, rng))),
        TypeInner::Array(array_ty, size) => Value::array(
            (0..*size).map(|_| generate_value_by_ty(array_ty, rng)),
            (**array_ty).clone(),
        ),
        TypeInner::List(list_ty, size_pow_2) => {
            let size = rng.random_range(0..size_pow_2.get());
            Value::list(
                (0..size).map(|_| generate_value_by_ty(list_ty, rng)),
                (**list_ty).clone(),
                *size_pow_2,
            )
        }
        _ => Value::unit(),
    }
}

pub fn generate_value_by_ty_iterative<R: Rng + ?Sized>(root_ty: &ResolvedType, rng: &mut R) -> Value {
    enum Step<'a> {
        Gen(&'a ResolvedType),
        AssembleEither(bool, ResolvedType),
        AssembleOption,
        AssembleTuple(usize),
        AssembleArray(usize, ResolvedType),
        AssembleList(usize, ResolvedType, NonZeroPow2Usize),
    }

    let mut tasks = vec![Step::Gen(root_ty)];
    let mut results: Vec<Value> = vec![];

    while let Some(task) = tasks.pop() {
        match task {
            Step::Gen(ty) => match ty.as_inner() {
                TypeInner::Boolean => results.push(Value::from(rng.random::<bool>())),
                TypeInner::UInt(x) => {
                    let val = match x {
                        UIntType::U1 => Value::u1(rng.random::<u8>() & 0x01),
                        UIntType::U2 => Value::u2(rng.random::<u8>() & 0x03),
                        UIntType::U4 => Value::u4(rng.random::<u8>() & 0x0F),
                        UIntType::U8 => Value::u8(rng.random::<u8>()),
                        UIntType::U16 => Value::u16(rng.random::<u16>()),
                        UIntType::U32 => Value::u32(rng.random::<u32>()),
                        UIntType::U64 => Value::u64(rng.random::<u64>()),
                        UIntType::U128 => Value::u128(rng.random::<u128>()),
                        UIntType::U256 => Value::u256(U256::from_byte_array(rng.random())),
                    };
                    results.push(val);
                }
                TypeInner::Either(left_ty, right_ty) => {
                    if rng.random::<bool>() {
                        tasks.push(Step::AssembleEither(true, (**right_ty).clone()));
                        tasks.push(Step::Gen(left_ty));
                    } else {
                        tasks.push(Step::AssembleEither(false, (**left_ty).clone()));
                        tasks.push(Step::Gen(right_ty));
                    }
                }
                TypeInner::Option(inner_ty) => {
                    if rng.random::<bool>() {
                        tasks.push(Step::AssembleOption);
                        tasks.push(Step::Gen(inner_ty));
                    } else {
                        results.push(Value::none((**inner_ty).clone()));
                    }
                }
                TypeInner::Tuple(types) => {
                    tasks.push(Step::AssembleTuple(types.len()));
                    for t in types.iter().rev() {
                        tasks.push(Step::Gen(t));
                    }
                }
                TypeInner::Array(inner_ty, size) => {
                    tasks.push(Step::AssembleArray(*size, (**inner_ty).clone()));
                    for _ in 0..*size {
                        tasks.push(Step::Gen(inner_ty));
                    }
                }
                TypeInner::List(inner_ty, max_pow2) => {
                    let len = rng.random_range(0..max_pow2.get());
                    tasks.push(Step::AssembleList(len, (**inner_ty).clone(), *max_pow2));
                    for _ in 0..len {
                        tasks.push(Step::Gen(inner_ty));
                    }
                }
                _ => results.push(Value::unit()),
            },

            Step::AssembleEither(is_left, other_ty) => {
                let val = results.pop().unwrap();
                results.push(if is_left {
                    Value::left(val, other_ty)
                } else {
                    Value::right(other_ty, val)
                });
            }
            Step::AssembleOption => {
                let val = results.pop().unwrap();
                results.push(Value::some(val));
            }
            Step::AssembleTuple(len) => {
                let items: Vec<_> = results.drain(results.len() - len..).collect();
                results.push(Value::tuple(items));
            }
            Step::AssembleArray(size, ty) => {
                let items: Vec<_> = results.drain(results.len() - size..).collect();
                results.push(Value::array(items, ty));
            }
            Step::AssembleList(len, ty, max) => {
                let items: Vec<_> = results.drain(results.len() - len..).collect();
                results.push(Value::list(items, ty, max));
            }
        }
    }

    results.pop().expect("Stack underflow")
}
