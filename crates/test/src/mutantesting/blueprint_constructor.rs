use crate::mutantesting::FuzzableProgram;
use crate::mutantesting::blueprint_constructor::dymmy_program::derived_dummy_program::{
    DummyProgramArguments, DummyProgramWitness,
};
use proptest::prelude::{BoxedStrategy, Strategy};
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;
use simplicityhl::elements::hashes::Hash;
use simplicityhl::{Arguments, WitnessValues};
use smplx_sdk::program::{ProgramFactory, ProgramTrait, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO};

#[derive(Clone, Debug)]
pub struct FinalTxMeta {
    program_input_idxs: Vec<usize>,
    program_output_idxs: Vec<usize>,
    network: SimplicityNetwork,
}

#[derive(Clone, Debug)]
pub struct BlueprintDraftConstructor {
    meta: FinalTxMeta,
    ft: FinalTransaction,
}

impl BlueprintDraftConstructor {
    pub fn new() -> Self {
        Self {
            meta: FinalTxMeta {
                program_input_idxs: vec![],
                program_output_idxs: vec![],
                network: SimplicityNetwork::default_regtest(),
            },
            ft: FinalTransaction::new(),
        }
    }

    pub fn add_program_input(mut self, amount: Option<u64>) -> Self {
        self.add_program_custom_input(amount, None, Some(RequiredSignature::None))
    }

    pub fn add_program_custom_input(
        mut self,
        amount: Option<u64>,
        outpoint: Option<simplicityhl::elements::OutPoint>,
        required_signature: Option<RequiredSignature>,
    ) -> Self {
        let last_input_idx = self.ft.inputs().len();
        if amount.is_none() {
            self.meta.program_input_idxs.push(last_input_idx);
        }

        let (prog, script) =
            dymmy_program::DummyProgram::build_program(DummyProgramArguments {}, &SimplicityNetwork::default_regtest());
        let mut txout =
            simplicityhl::elements::TxOut::new_fee(amount.unwrap_or_default(), self.meta.network.policy_asset());
        txout.script_pubkey = script;

        let partial_input = PartialInput::new(UTXO {
            outpoint: outpoint.unwrap_or(simplicityhl::elements::OutPoint::new(
                simplicityhl::elements::Txid::all_zeros(),
                0,
            )),
            txout,
            secrets: None,
        });
        let program_input = ProgramInput::new(
            Box::new(prog.as_ref().as_ref().clone()),
            DummyProgramWitness {}.build_witness(),
        );

        self.ft.add_program_input(
            partial_input,
            program_input,
            required_signature.unwrap_or(RequiredSignature::None),
        );
        self
    }

    pub fn randomize_input_value(mut self, idx: usize) -> Self {
        self.meta.program_input_idxs.push(idx);
        self
    }

    pub fn randomize_output_value(mut self, idx: usize) -> Self {
        self.meta.program_output_idxs.push(idx);
        self
    }

    pub fn add_static_input(mut self, partial_input: PartialInput, required_sig: RequiredSignature) -> Self {
        self.ft.add_input(partial_input, required_sig);
        self
    }

    pub fn add_static_output(mut self, partial_output: PartialOutput) -> Self {
        self.ft.add_output(partial_output);
        self
    }

    pub fn insert_real_program_values<Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static>(
        &self,
        context: &mut TestRunner,
        args_wit_strategy: &BoxedStrategy<(Arguments, WitnessValues)>,
    ) -> (Arguments, WitnessValues, FinalTransaction) {
        let mut ft = self.ft.clone();
        let (args, wit) = args_wit_strategy.new_tree(context).unwrap().current();

        for idx in self.meta.program_input_idxs.iter() {
            let prog_instance = Program::instantiate_program(args.clone());
            let sdk_program = prog_instance.as_ref().as_ref().clone();
            ft.inputs_mut()[*idx].program_input = Some(ProgramInput::new(Box::new(sdk_program), wit.clone()));
        }
        for idx in self.meta.program_output_idxs.iter() {
            let (_, script) = Program::build_program(args.clone(), &self.meta.network);
            ft.outputs_mut()[*idx].script_pubkey = script;
        }

        (args, wit, ft)
    }
}

pub mod dymmy_program {
    use simplicityhl::Arguments;
    use simplicityhl::elements::Script;
    use simplicityhl::elements::secp256k1_zkp::XOnlyPublicKey;
    use smplx_sdk::program::{ArgumentsTrait, Program};
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
        #[must_use]
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

            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct DummyProgramWitness {}
            impl DummyProgramWitness {
                #[doc = r" Build struct from Simplicity `WitnessValues`."]
                #[doc = r""]
                #[doc = r" # Errors"]
                #[doc = r""]
                #[doc = r" Returns error if any required witness is missing, has the wrong type, or has an invalid value."]
                pub fn from_witness(witness: &WitnessValues) -> Result<Self, String> {
                    Ok(Self {})
                }
                #[doc = r" Generate a random Witness struct instance using the provided RNG."]
                pub fn generate_witness_raw<R: RngCore + ?Sized>(rng: &mut R) -> Self {
                    DummyProgramWitness {}
                }
            }
            impl smplx_sdk::program::WitnessTrait for DummyProgramWitness {
                #[doc = r" Build Simplicity witness values for contract execution."]
                #[must_use]
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
            impl core::default::Default for DummyProgramWitness {
                fn default() -> Self {
                    DummyProgramWitness {}
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
            use rand::RngCore;
            use simplicityhl::Arguments;
            use smplx_sdk::program::ArgumentsTrait;
            use std::collections::HashMap;
            #[derive(Debug, Clone, PartialEq, Eq)]
            pub struct DummyProgramArguments {}
            impl DummyProgramArguments {
                #[doc = r" Build struct from Simplicity `Arguments`."]
                #[doc = r""]
                #[doc = r" # Errors"]
                #[doc = r""]
                #[doc = r" Returns error if any required witness is missing, has the wrong type, or has an invalid value."]
                pub fn from_arguments(args: &Arguments) -> Result<Self, String> {
                    Ok(Self {})
                }
                #[doc = r" Generate a random Arguments struct instance using the provided RNG."]
                pub fn generate_arguments_raw<R: RngCore + ?Sized>(rng: &mut R) -> Self {
                    DummyProgramArguments {}
                }
            }
            impl smplx_sdk::program::ArgumentsTrait for DummyProgramArguments {
                #[doc = r" Build Simplicity arguments for contract instantiation."]
                #[must_use]
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
            impl core::default::Default for DummyProgramArguments {
                fn default() -> Self {
                    DummyProgramArguments {}
                }
            }
            impl From<DummyProgramArguments> for simplicityhl::Arguments {
                fn from(val: DummyProgramArguments) -> simplicityhl::Arguments {
                    val.build_arguments()
                }
            }
        }
        mod program_helpers {
            use super::super::DummyProgram;
            use simplicityhl::Arguments;
            use smplx_sdk::program::ProgramFactory;
            impl ProgramFactory<DummyProgram> for DummyProgram {
                fn instantiate_program(args: impl Into<Arguments>) -> Box<DummyProgram> {
                    Box::new(DummyProgram::new(args))
                }
            }
        }
    }
}
