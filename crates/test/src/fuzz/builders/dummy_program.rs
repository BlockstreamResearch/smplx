use simplicityhl::Arguments;
use simplicityhl::elements::Script;
use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;

use smplx_sdk::program::Program;
use smplx_sdk::provider::SimplicityNetwork;

pub struct DummyProgram {
    program: Program,
}
impl DummyProgram {
    pub const SOURCE: &'static str = derived_dummy_program::DUMMY_PROGRAM_CONTRACT_SOURCE;

    #[must_use]
    pub fn new(arguments: impl Into<Arguments>) -> Self {
        Self {
            program: Program::new(Self::SOURCE, arguments),
        }
    }

    #[must_use]
    pub fn with_taproot_pubkey(mut self, pub_key: XOnlyPublicKey) -> Self {
        self.program = self.program.with_taproot_pubkey(pub_key);
        self
    }

    #[must_use]
    pub fn with_storage_capacity(mut self, capacity: usize) -> Self {
        self.program = self.program.with_storage_capacity(capacity);
        self
    }

    pub fn set_storage_at(&mut self, index: usize, new_value: [u8; 32]) {
        self.program.set_storage_at(index, new_value);
    }

    #[must_use]
    pub fn get_storage_len(&self) -> usize {
        self.program.get_storage_len()
    }

    #[must_use]
    pub fn get_storage(&self) -> &[[u8; 32]] {
        self.program.get_storage()
    }

    #[must_use]
    pub fn get_storage_at(&self, index: usize) -> [u8; 32] {
        self.program.get_storage_at(index)
    }

    #[must_use]
    pub fn get_script_pubkey(&self, network: &SimplicityNetwork) -> Script {
        self.program.get_script_pubkey(network)
    }

    #[must_use]
    pub fn get_script_hash(&self, network: &SimplicityNetwork) -> [u8; 32] {
        self.program.get_script_hash(network)
    }
}

impl AsRef<Program> for DummyProgram {
    fn as_ref(&self) -> &Program {
        &self.program
    }
}

impl AsMut<Program> for DummyProgram {
    fn as_mut(&mut self) -> &mut Program {
        &mut self.program
    }
}

pub mod derived_dummy_program {
    pub const DUMMY_PROGRAM_CONTRACT_SOURCE: &str = "mod unit_0 {\n    fn main() {\n    assert!(true);\n    }\n}\n";

    pub use build_witness::*;

    mod build_witness {
        use rand::RngCore;
        use simplicityhl::WitnessValues;
        use smplx_sdk::program::WitnessTrait;
        use std::collections::HashMap;

        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        pub struct DummyProgramWitness {}

        impl DummyProgramWitness {
            ///Build struct from Simplicity `WitnessValues`.
            ///
            ///# Errors
            ///
            ///Returns error if any required witness is missing, has the wrong type, or has an invalid value.
            pub fn from_witness(_witness: &WitnessValues) -> Result<Self, String> {
                Ok(Self {})
            }

            ///Generate a random Witness struct instance using the provided RNG.
            pub fn generate_witness_raw<R: RngCore + ?Sized>(_rng: &mut R) -> Self {
                DummyProgramWitness {}
            }
        }

        impl smplx_sdk::program::WitnessTrait for DummyProgramWitness {
            /// Build Simplicity witness values for contract execution.
            fn build_witness(&self) -> simplicityhl::WitnessValues {
                simplicityhl::WitnessValues::from(HashMap::from([]))
            }
        }

        impl serde::Serialize for DummyProgramWitness {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.build_witness().serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for DummyProgramWitness {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let x = simplicityhl::WitnessValues::deserialize(deserializer)?;
                Self::from_witness(&x).map_err(serde::de::Error::custom)
            }
        }

        impl smplx_sdk::program::RandomWitness for DummyProgramWitness {
            fn generate_witness(rng: &mut dyn RngCore) -> simplicityhl::WitnessValues {
                Self::generate_witness_raw(rng).build_witness()
            }
        }

        impl From<DummyProgramWitness> for simplicityhl::WitnessValues {
            fn from(val: DummyProgramWitness) -> simplicityhl::WitnessValues {
                val.build_witness()
            }
        }
    }

    pub use build_arguments::*;

    mod build_arguments {
        use std::collections::HashMap;

        use simplicityhl::Arguments;

        use rand::RngCore;

        use smplx_sdk::program::ArgumentsTrait;
        #[derive(Debug, Clone, PartialEq, Eq, Default)]
        pub struct DummyProgramArguments {}

        impl DummyProgramArguments {
            ///Build struct from Simplicity `Arguments`.
            ///
            ///# Errors
            ///
            ///Returns error if any required witness is missing, has the wrong type, or has an invalid value.
            pub fn from_arguments(_args: &Arguments) -> Result<Self, String> {
                Ok(Self {})
            }

            /// Generate a random Arguments struct instance using the provided RNG.
            pub fn generate_arguments_raw<R: RngCore + ?Sized>(_rng: &mut R) -> Self {
                DummyProgramArguments {}
            }
        }

        impl smplx_sdk::program::ArgumentsTrait for DummyProgramArguments {
            /// Build Simplicity arguments for contract instantiation.
            fn build_arguments(&self) -> simplicityhl::Arguments {
                simplicityhl::Arguments::from(HashMap::from([]))
            }
        }

        impl serde::Serialize for DummyProgramArguments {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                self.build_arguments().serialize(serializer)
            }
        }

        impl<'de> serde::Deserialize<'de> for DummyProgramArguments {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let x = simplicityhl::Arguments::deserialize(deserializer)?;
                Self::from_arguments(&x).map_err(serde::de::Error::custom)
            }
        }

        impl smplx_sdk::program::RandomArguments for DummyProgramArguments {
            fn generate_arguments(rng: &mut dyn RngCore) -> simplicityhl::Arguments {
                Self::generate_arguments_raw(rng).build_arguments()
            }
        }

        impl From<DummyProgramArguments> for simplicityhl::Arguments {
            fn from(val: DummyProgramArguments) -> simplicityhl::Arguments {
                val.build_arguments()
            }
        }
    }

    mod program_helpers {
        use simplicityhl::Arguments;

        use smplx_sdk::program::ProgramFactory;

        use super::super::DummyProgram;

        impl ProgramFactory<DummyProgram> for DummyProgram {
            fn instantiate_program(args: impl Into<Arguments>) -> Box<DummyProgram> {
                Box::new(DummyProgram::new(args))
            }
        }
    }
}
