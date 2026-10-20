use proptest::test_runner::TestRng;

use rand::RngExt;

use simplicityhl::Value;
use simplicityhl::num::U256;
use simplicityhl::types::UIntType;
use simplicityhl::value::ValueConstructible;

const BOUNDARY_POWERS: &[usize] = &[4, 5, 6, 7, 8, 10, 12, 15, 16, 31, 32, 63, 64, 127, 128, 255];
const BOUNDARY_SMALL: &[u128] = &[0, 1, 2, 100, 1000];

/// Big-endian 256-bit unsigned integer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Word([u8; 32]);

impl From<U256> for Word {
    fn from(value: U256) -> Self {
        Self(value.to_byte_array())
    }
}

impl Word {
    pub(crate) fn from_u128(value: u128) -> Self {
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

    pub(crate) fn random(bits: usize, rng: &mut TestRng) -> Self {
        Self(rng.random()).masked(bits)
    }

    pub(crate) fn boundaries(bits: usize) -> Vec<Self> {
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

    pub(crate) fn fits(&self, bits: usize) -> bool {
        *self <= Self::max(bits)
    }

    pub(crate) fn add_one(mut self) -> Option<Self> {
        for byte in self.0.iter_mut().rev() {
            let (sum, overflow) = byte.overflowing_add(1);
            *byte = sum;

            if !overflow {
                return Some(self);
            }
        }

        None
    }

    pub(crate) fn sub_one(mut self) -> Option<Self> {
        for byte in self.0.iter_mut().rev() {
            let (difference, underflow) = byte.overflowing_sub(1);
            *byte = difference;

            if !underflow {
                return Some(self);
            }
        }

        None
    }

    pub(crate) fn to_byte_array(self) -> [u8; 32] {
        self.0
    }

    /// The word as a `u64`, `None` when it does not fit.
    pub(crate) fn to_u64(self) -> Option<u64> {
        self.fits(64)
            .then(|| u64::from_be_bytes(self.0[24..].try_into().expect("8 bytes")))
    }

    pub(crate) fn to_value(self, uint: UIntType) -> Value {
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
    use super::*;

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
}
