use crate::arguments::ArgumentsTrait;
use crate::program::Program;

use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;

// TODO macro
pub struct P2PK<'a> {
    program: Program<'a>,
}

impl<'a> P2PK<'a> {
    pub const SOURCE: &'static str = include_str!("./simf/p2pk.simf");

    pub fn new(public_key: &'a XOnlyPublicKey, arguments: &'a impl ArgumentsTrait) -> Self {
        Self {
            program: Program::new(Self::SOURCE, public_key, arguments),
        }
    }

    pub fn get_program(&self) -> &Program<'a> {
        &self.program
    }

    pub fn get_program_mut(&mut self) -> &mut Program<'a> {
        &mut self.program
    }
}

pub mod p2pk_build {
    use crate::arguments::ArgumentsTrait;
    use crate::witness::WitnessTrait;
    use simplicityhl::num::U256;
    use simplicityhl::str::WitnessName;
    use simplicityhl::types::TypeConstructible;
    use simplicityhl::value::UIntValue;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, ResolvedType, Value, WitnessValues};
    use std::collections::HashMap;

    pub struct P2PKWitness {
        pub signature: [u8; 64usize],
    }

    pub struct P2PKArguments {
        pub public_key: [u8; 32],
    }

    impl WitnessTrait for P2PKWitness {
        fn build_witness(&self) -> WitnessValues {
            WitnessValues::from(HashMap::from([(WitnessName::from_str_unchecked("SIGNATURE"), {
                let elements = [
                    Value::from(UIntValue::U8(self.signature[0])),
                    Value::from(UIntValue::U8(self.signature[1])),
                    Value::from(UIntValue::U8(self.signature[2])),
                    Value::from(UIntValue::U8(self.signature[3])),
                    Value::from(UIntValue::U8(self.signature[4])),
                    Value::from(UIntValue::U8(self.signature[5])),
                    Value::from(UIntValue::U8(self.signature[6])),
                    Value::from(UIntValue::U8(self.signature[7])),
                    Value::from(UIntValue::U8(self.signature[8])),
                    Value::from(UIntValue::U8(self.signature[9])),
                    Value::from(UIntValue::U8(self.signature[10])),
                    Value::from(UIntValue::U8(self.signature[11])),
                    Value::from(UIntValue::U8(self.signature[12])),
                    Value::from(UIntValue::U8(self.signature[13])),
                    Value::from(UIntValue::U8(self.signature[14])),
                    Value::from(UIntValue::U8(self.signature[15])),
                    Value::from(UIntValue::U8(self.signature[16])),
                    Value::from(UIntValue::U8(self.signature[17])),
                    Value::from(UIntValue::U8(self.signature[18])),
                    Value::from(UIntValue::U8(self.signature[19])),
                    Value::from(UIntValue::U8(self.signature[20])),
                    Value::from(UIntValue::U8(self.signature[21])),
                    Value::from(UIntValue::U8(self.signature[22])),
                    Value::from(UIntValue::U8(self.signature[23])),
                    Value::from(UIntValue::U8(self.signature[24])),
                    Value::from(UIntValue::U8(self.signature[25])),
                    Value::from(UIntValue::U8(self.signature[26])),
                    Value::from(UIntValue::U8(self.signature[27])),
                    Value::from(UIntValue::U8(self.signature[28])),
                    Value::from(UIntValue::U8(self.signature[29])),
                    Value::from(UIntValue::U8(self.signature[30])),
                    Value::from(UIntValue::U8(self.signature[31])),
                    Value::from(UIntValue::U8(self.signature[32])),
                    Value::from(UIntValue::U8(self.signature[33])),
                    Value::from(UIntValue::U8(self.signature[34])),
                    Value::from(UIntValue::U8(self.signature[35])),
                    Value::from(UIntValue::U8(self.signature[36])),
                    Value::from(UIntValue::U8(self.signature[37])),
                    Value::from(UIntValue::U8(self.signature[38])),
                    Value::from(UIntValue::U8(self.signature[39])),
                    Value::from(UIntValue::U8(self.signature[40])),
                    Value::from(UIntValue::U8(self.signature[41])),
                    Value::from(UIntValue::U8(self.signature[42])),
                    Value::from(UIntValue::U8(self.signature[43])),
                    Value::from(UIntValue::U8(self.signature[44])),
                    Value::from(UIntValue::U8(self.signature[45])),
                    Value::from(UIntValue::U8(self.signature[46])),
                    Value::from(UIntValue::U8(self.signature[47])),
                    Value::from(UIntValue::U8(self.signature[48])),
                    Value::from(UIntValue::U8(self.signature[49])),
                    Value::from(UIntValue::U8(self.signature[50])),
                    Value::from(UIntValue::U8(self.signature[51])),
                    Value::from(UIntValue::U8(self.signature[52])),
                    Value::from(UIntValue::U8(self.signature[53])),
                    Value::from(UIntValue::U8(self.signature[54])),
                    Value::from(UIntValue::U8(self.signature[55])),
                    Value::from(UIntValue::U8(self.signature[56])),
                    Value::from(UIntValue::U8(self.signature[57])),
                    Value::from(UIntValue::U8(self.signature[58])),
                    Value::from(UIntValue::U8(self.signature[59])),
                    Value::from(UIntValue::U8(self.signature[60])),
                    Value::from(UIntValue::U8(self.signature[61])),
                    Value::from(UIntValue::U8(self.signature[62])),
                    Value::from(UIntValue::U8(self.signature[63])),
                ];
                Value::array(elements, ResolvedType::u8())
            })]))
        }
    }

    impl ArgumentsTrait for P2PKArguments {
        fn build_arguments(&self) -> Arguments {
            Arguments::from(HashMap::from([(
                WitnessName::from_str_unchecked("PUBLIC_KEY"),
                Value::from(UIntValue::U256(U256::from_byte_array(self.public_key))),
            )]))
        }
    }
}
