use rand::Rng;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::num::U256;
use simplicityhl::types::{TypeInner, UIntType};
use simplicityhl::value::ValueConstructible;
use simplicityhl::{ResolvedType, Value};
use smplx_sdk::signer::{Signer, SignerError};
use smplx_sdk::transaction::FinalTransaction;

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

#[inline]
pub fn sign_or_extract(
    signer: &Option<Signer>,
    ft: &FinalTransaction,
) -> Result<PartiallySignedTransaction, SignerError> {
    match signer.as_ref() {
        None => Ok(ft.extract_pst().0),
        Some(signer) => signer.sign_tx_raw(ft),
    }
}
