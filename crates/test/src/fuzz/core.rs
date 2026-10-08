use std::fmt::Debug;
use std::marker::PhantomData;

use proptest::prelude::{BoxedStrategy, TestCaseError};
use proptest::strategy::Strategy;
use proptest::test_runner::{Config as ProptestConfig, TestRunner};

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::{ProgramFactory, ProgramTrait, RandomArguments, RandomWitness};
use smplx_sdk::signer::Signer;

use crate::fuzz::FuzzableProgram;
use crate::fuzz::args_strategy::ArgsStrategyBuilder;
use crate::fuzz::fuzz_check::{Expect, FuzzExecutionCheck, ProgramCheck, ProgramExecResult};
use crate::fuzz::fuzz_transaction::{FuzzTransaction, ProgramTarget};

pub struct SimplexFuzzEngine<Program, Args, Wit> {
    pub config: ProptestConfig,
    pub signer: Signer,
    pub strategy: Option<BoxedStrategy<(Arguments, WitnessValues)>>,
    pub initial_transaction: Option<FuzzTransaction>,
    pub program_check: Option<FuzzExecutionCheck>,
    pub _placeholder: PhantomData<(Program, Args, Wit)>,
}

impl<Program, Args, Wit> SimplexFuzzEngine<Program, Args, Wit>
where
    Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
    Args: Into<Arguments> + RandomArguments + Debug + Clone + 'static,
    Wit: Into<WitnessValues> + RandomWitness + Debug + Clone + 'static,
{
    pub fn new(config: ProptestConfig, signer: Signer) -> SimplexFuzzEngine<Program, Args, Wit> {
        SimplexFuzzEngine {
            config,
            signer,
            strategy: None,
            initial_transaction: None,
            program_check: None,
            _placeholder: Default::default(),
        }
    }

    #[must_use]
    pub fn with_custom_config(mut self, config: ProptestConfig) -> Self {
        self.config = config;

        self
    }

    #[must_use]
    pub fn with_custom_signer(mut self, signer: Signer) -> Self {
        self.signer = signer;

        self
    }

    #[must_use]
    pub fn with_custom_strategy(
        mut self,
        strategy: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
    ) -> Self {
        self.strategy = Some(strategy.boxed());

        self
    }

    #[must_use]
    pub fn with_custom_transaction(mut self, transaction: FuzzTransaction) -> Self {
        self.initial_transaction = Some(transaction);

        self
    }

    #[must_use]
    pub fn with_custom_check(mut self, check: FuzzExecutionCheck) -> Self {
        self.program_check = Some(check);

        self
    }

    pub fn run(self) {
        let mut runner = TestRunner::new(self.config);

        let strategy = match self.strategy {
            None => ArgsStrategyBuilder::<Args, Wit>::new().build(),
            Some(strategy) => strategy,
        };
        let program_check = match self.program_check {
            None => FuzzExecutionCheck::new(runner.config().test_name.unwrap_or("simplex_fuzz"), Expect::Ok),
            Some(check) => check,
        };
        let initial_transaction = match self.initial_transaction {
            None => FuzzTransaction::try_default().expect("Shouldn't fail. Please report a bug"),
            Some(tx) => tx,
        };

        let result = runner.run(&strategy.no_shrink(), |(arguments, witness)| {
            Self::fuzz(&self.signer, &initial_transaction, &program_check, arguments, witness)
        });

        match result {
            Ok(()) => {}
            Err(proptest::test_runner::TestError::Fail(reason, (args, wit))) => {
                panic!("Program failed with these arguments: {args} and witness: {wit}, reason: `{reason}`");
            }
            Err(err) => {
                panic!("Fuzzing aborted: {err}");
            }
        }
    }

    /// Extracted helper that performs exactly one isolated test run.
    fn fuzz(
        signer: &Signer,
        initial_tx: &FuzzTransaction,
        program_check: &impl ProgramCheck<Program, Args, Wit>,
        arguments: Arguments,
        witness: WitnessValues,
    ) -> Result<(), TestCaseError> {
        let (program, script) = Program::build_program(arguments.clone(), &signer.get_network());

        let final_transaction = initial_tx
            .prepare_transaction(program.as_ref().as_ref(), &script, &arguments, &witness)
            .map_err(|error| TestCaseError::fail(format!("failed to prepare fuzz transaction: {error}")))?;

        let (pst, signed_witnesses) = signer
            .sign_tx(&final_transaction)
            .map_err(|error| TestCaseError::fail(format!("failed to sign: {error}")))?;

        // Iterate over program inputs to check contract execution
        for target in initial_tx.targets().iter().copied() {
            let ProgramTarget::Input(input_index) = target else {
                continue;
            };

            let signed_witness = signed_witnesses.get(&input_index).unwrap_or(&witness);

            let exec_result: ProgramExecResult =
                program
                    .as_ref()
                    .as_ref()
                    .execute(&pst, signed_witness, input_index, &*signer.get_network());

            if let Err(error) = program_check.call(signer, &pst, &arguments, signed_witness, input_index, exec_result) {
                return Err(TestCaseError::fail(format!(
                    "{error}, args: {arguments}, wit: {signed_witness}"
                )));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::Just;

    use smplx_sdk::global::Verbosity;
    use smplx_sdk::program::{Program, RandomArguments, RandomWitness};
    use smplx_sdk::provider::EsploraProvider;
    use smplx_sdk::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

    use crate::config::{DEFAULT_TEST_MNEMONIC, EsploraConfig, FuzzConfig, TestConfig};
    use crate::context::{FuzzMode, TestContext};
    use crate::error::FuzzError;

    use super::*;

    #[derive(Clone)]
    struct DummyProgram(Program);

    impl AsRef<Program> for DummyProgram {
        fn as_ref(&self) -> &Program {
            &self.0
        }
    }

    impl ProgramFactory<DummyProgram> for DummyProgram {
        fn instantiate_program(args: impl Into<Arguments>) -> Box<DummyProgram> {
            Box::new(DummyProgram(Program::new("fn main() { assert!(true); }", args)))
        }
    }

    #[derive(Clone, Debug)]
    struct EmptyArgs;

    impl From<EmptyArgs> for Arguments {
        fn from(_: EmptyArgs) -> Self {
            Self::default()
        }
    }

    impl From<EmptyArgs> for WitnessValues {
        fn from(_: EmptyArgs) -> Self {
            Self::default()
        }
    }

    impl RandomArguments for EmptyArgs {
        fn generate_arguments(_: &mut dyn rand::RngCore) -> Arguments {
            Arguments::default()
        }
    }

    impl RandomWitness for EmptyArgs {
        fn generate_witness(_: &mut dyn rand::RngCore) -> WitnessValues {
            WitnessValues::default()
        }
    }

    fn build_test_engine(context: TestContext<FuzzMode>) -> SimplexFuzzEngine<DummyProgram, EmptyArgs, EmptyArgs> {
        let mut final_tx = FinalTransaction::new();
        final_tx.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);

        let fuzz_tx = FuzzTransaction::new(final_tx, [ProgramTarget::Input(0)]).unwrap();
        context.build(Just((Arguments::default(), WitnessValues::default())), fuzz_tx)
    }

    #[test]
    fn context_preserves_fuzz_network_signer_and_config() {
        let config = TestConfig {
            esplora: Some(EsploraConfig {
                url: "http://localhost:3000".to_string(),
                network: "LiquidTestnet".to_string(),
            }),
            fuzz: Some(FuzzConfig {
                cases: Some(17),
                max_global_rejects: Some(23),
                max_local_rejects: Some(29),
                network: Some("Liquid".to_string()),
            }),
            verbosity: Verbosity::Trace,
            ..Default::default()
        };

        let test_name = "crate::fuzz_test";
        let source_file = "src/fuzz_test.any";
        let context = TestContext::from_config(config)
            .unwrap()
            .fuzz(test_name, source_file)
            .unwrap();

        let engine = build_test_engine(context);
        let runner_config = engine.runner.config();
        let network = SimplicityNetwork::Liquid;

        assert_eq!(engine.context.network, network);

        let signer = engine.context.signer.as_ref().unwrap();
        assert_eq!(signer.get_address().params, network.address_params());
        assert!(signer.get_provider().is_err());
        assert_eq!(runner_config.cases, 17);
        assert_eq!(runner_config.max_global_rejects, 23);
        assert_eq!(runner_config.max_local_rejects, 29);
        assert_eq!(runner_config.verbose, 2);
        assert_eq!(runner_config.test_name, Some(test_name));
        assert!(!runner_config.fork);
        assert_eq!(runner_config.max_shrink_iters, 0);
        assert_eq!(runner_config.source_file, Some(source_file));
    }

    #[test]
    fn explicit_network_without_signer_builds_unsigned_engine() {
        let mut test_context = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz("crate::unsigned_fuzz_test", file!())
            .unwrap();
        assert!(test_context.get_default_signer().is_none());
        assert!(test_context.random_signer().is_none());

        let network = SimplicityNetwork::default_regtest();
        test_context.set_network(network).unwrap();

        let engine = build_test_engine(test_context);
        assert_eq!(engine.context.network, network);
        assert!(engine.context.signer.is_none());

        let context = FuzzContext { signer: None, network };

        let (_, witnesses) = SimplexFuzzEngine::<DummyProgram, EmptyArgs, EmptyArgs>::sign_or_extract(
            &context,
            &FinalTransaction::new(),
        )
        .unwrap();
        assert!(witnesses.is_empty());
    }

    #[test]
    fn setters_override_fuzz_configuration_and_preserve_custom_signer() {
        let config = TestConfig {
            fuzz: Some(FuzzConfig {
                cases: Some(17),
                max_global_rejects: Some(23),
                max_local_rejects: Some(29),
                network: Some("Liquid".to_string()),
            }),
            ..Default::default()
        };

        let mut context = TestContext::from_config(config)
            .unwrap()
            .fuzz("crate::custom_fuzz_test", file!())
            .unwrap();

        let network = SimplicityNetwork::LiquidTestnet;
        let custom_signer = Signer::new(
            DEFAULT_TEST_MNEMONIC,
            Box::new(EsploraProvider::new("http://localhost:3001".to_string(), network)),
        );

        let public_key = custom_signer.get_schnorr_public_key();

        context.set_custom_signer(custom_signer);
        context.set_network(network).unwrap();
        context.set_cases(31);
        context.set_max_global_rejects(37);
        context.set_max_local_rejects(41);

        let engine = build_test_engine(context);

        assert_eq!(engine.context.network, network);

        let signer = engine.context.signer.as_ref().unwrap();
        assert_eq!(signer.get_schnorr_public_key(), public_key);
        assert_eq!(signer.get_provider().unwrap().get_network(), &network);

        assert_eq!(engine.runner.config().cases, 31);
        assert_eq!(engine.runner.config().max_global_rejects, 37);
        assert_eq!(engine.runner.config().max_local_rejects, 41);
    }

    #[test]
    fn signer_network_mismatch_error() {
        let mut context = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz("crate::mismatched_fuzz_network", file!())
            .unwrap();

        let signer_network = SimplicityNetwork::Liquid;
        context.set_custom_signer(Signer::from_mnemonic(DEFAULT_TEST_MNEMONIC, signer_network));

        let network = SimplicityNetwork::LiquidTestnet;
        assert_eq!(
            context.set_network(network),
            Err(FuzzError::SignerNetworkMismatch {
                network: format!("{:?}", network),
                signer_network: format!("{:?}", signer_network),
            })
        );

        let engine = build_test_engine(context);
        assert_eq!(engine.context.network, signer_network);
        assert_eq!(*engine.context.signer.unwrap().get_network(), signer_network);
    }

    #[test]
    fn unsigned_extraction_preserves_program_witnesses_at_their_input_indices() {
        use simplicityhl::TemplateProgramWitness;
        use simplicityhl::value::{Value, ValueConstructible};

        let context = FuzzContext {
            signer: None,
            network: SimplicityNetwork::default_regtest(),
        };
        let name = TemplateProgramWitness::parameter_from_str("VALUE");
        let witness = WitnessValues::from_map(HashMap::from([(name.clone(), Value::u8(42))]));
        let program = Program::new("fn main() { assert!(true); }", Arguments::default());
        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);
        transaction.add_program_input(
            PartialInput::new(UTXO::default()),
            ProgramInput::new(Box::new(program), witness.clone()),
            RequiredSignature::None,
        );

        let (pst, witnesses) =
            SimplexFuzzEngine::<DummyProgram, EmptyArgs, EmptyArgs>::sign_or_extract(&context, &transaction).unwrap();

        assert_eq!(pst.inputs().len(), 2);
        assert_eq!(witnesses.len(), 1);
        assert_eq!(witnesses[&1].get(&name), witness.get(&name));
        assert!(pst.inputs()[1].final_script_witness.is_none());
    }
}
