use crate::program::ArgumentsTrait;
use crate::program::Program;

use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;

// TODO macro
pub struct P2PK {
    program: Program,
}

impl P2PK {
    pub const SOURCE: &'static str = include_str!("./simf/p2pk.simf");

    pub fn new(public_key: XOnlyPublicKey, arguments: impl ArgumentsTrait + 'static) -> Self {
        Self {
            program: Program::new(Self::SOURCE, public_key, Box::new(arguments)),
        }
    }

    pub fn get_program(&self) -> &Program {
        &self.program
    }

    pub fn get_program_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}

pub mod p2pk_build {
    use crate::program::ArgumentsTrait;
    use crate::program::WitnessTrait;
    use simplicityhl::num::U256;
    use simplicityhl::str::WitnessName;
    use simplicityhl::value::UIntValue;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, Value, WitnessValues};
    use std::collections::HashMap;

    #[derive(Clone)]
    pub struct P2PKWitness {
        pub signature: [u8; 64usize],
    }

    #[derive(Clone)]
    pub struct P2PKArguments {
        pub public_key: [u8; 32],
    }

    impl WitnessTrait for P2PKWitness {
        fn build_witness(&self) -> WitnessValues {
            WitnessValues::from(HashMap::from([(
                WitnessName::from_str_unchecked("SIGNATURE"),
                Value::byte_array(self.signature),
            )]))
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
