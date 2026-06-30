use crate::fuzz::FuzzableProgram;
use crate::fuzz::builders::dummy_program;
use crate::fuzz::builders::dummy_program::derived_dummy_program::{DummyProgramArguments, DummyProgramWitness};

use simplicityhl::elements::hashes::Hash;
use simplicityhl::{Arguments, WitnessValues};

use proptest::prelude::{BoxedStrategy, Strategy};
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;

use smplx_sdk::program::{ProgramFactory, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO};

#[derive(Clone, Debug)]
pub struct FinalTxMeta {
    program_input_idxs: Vec<usize>,
    program_output_idxs: Vec<usize>,
    network: SimplicityNetwork,
}

#[derive(Clone, Debug)]
pub struct FinalTransactionBuilder {
    meta: FinalTxMeta,
    ft: FinalTransaction,
}

impl Default for FinalTransactionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FinalTransactionBuilder {
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

    pub fn add_program_input(self, amount: Option<u64>) -> Self {
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
            dummy_program::DummyProgram::build_program(DummyProgramArguments {}, &SimplicityNetwork::default_regtest());
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

    pub fn get_inputs_to_check(&self) -> &[usize] {
        &self.meta.program_input_idxs
    }

    pub fn get_strategies_to_make_initial_ft_valid(&self) -> Vec<BoxedStrategy<FinalTransaction>> {
        vec![]
    }

    pub fn insert_real_program_values<Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static>(
        &self,
        context: &mut TestRunner,
        args_wit_strategy: &BoxedStrategy<(Arguments, WitnessValues)>,
    ) -> (Arguments, WitnessValues, FinalTransaction) {
        let mut ft = self.ft.clone();
        let (args, wit) = args_wit_strategy.new_tree(context).unwrap().current();
        let (prog_instance, script) = Program::build_program(args.clone(), &self.meta.network);

        for idx in self.meta.program_input_idxs.iter() {
            let sdk_program = prog_instance.as_ref().as_ref().clone();
            let edit_input = &mut ft.inputs_mut()[*idx];
            edit_input.program_input = Some(ProgramInput::new(Box::new(sdk_program), wit.clone()));
            edit_input.partial_input.witness_utxo.script_pubkey = script.clone();
        }
        for idx in self.meta.program_output_idxs.iter() {
            ft.outputs_mut()[*idx].script_pubkey = script.clone();
        }

        (args, wit, ft)
    }
}
