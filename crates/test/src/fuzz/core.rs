use std::any::Any;
use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use proptest::prelude::{BoxedStrategy, TestCaseError};
use proptest::strategy::Strategy;
use proptest::test_runner::{Config as ProptestConfig, FailurePersistence, PersistedSeed, TestError, TestRunner};

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::program::ProgramTrait;
use smplx_sdk::signer::Signer;

use crate::fuzz::FuzzableProgram;
use crate::fuzz::args_strategy::ArgsStrategyBuilder;
use crate::fuzz::fuzz_transaction::{FuzzTransaction, ProgramTarget};
use crate::fuzz::outcome::{Outcome, SetupError};
use crate::fuzz::spec::{Expect, FuzzCase};

type Spec<P> = dyn Fn(&FuzzCase<'_, P>) -> Expect;

enum CaseFailure {
    Setup(SetupError),
    Finding(String),
}

pub struct SimplexFuzzEngine<Program: FuzzableProgram> {
    pub config: ProptestConfig,
    pub signer: Signer,
    pub strategy: Option<BoxedStrategy<(Arguments, WitnessValues)>>,
    pub initial_transaction: Option<FuzzTransaction>,
    pub spec: Option<Box<Spec<Program>>>,
}

impl<Program: FuzzableProgram> SimplexFuzzEngine<Program> {
    pub fn new(config: ProptestConfig, signer: Signer) -> SimplexFuzzEngine<Program> {
        SimplexFuzzEngine {
            config,
            signer,
            strategy: None,
            initial_transaction: None,
            spec: None,
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

    /// Sets what should happen to each spend. Without a spec, every spend is expected to be accepted.
    #[must_use]
    pub fn spec(mut self, spec: impl Fn(&FuzzCase<'_, Program>) -> Expect + 'static) -> Self {
        self.spec = Some(Box::new(spec));

        self
    }

    pub fn run(self) {
        let setup_error = Arc::new(Mutex::new(None));

        let mut config = self.config;
        config.failure_persistence = config.failure_persistence.map(|inner| {
            Box::new(SkipSetupErrors {
                inner,
                setup_error: setup_error.clone(),
            }) as Box<dyn FailurePersistence>
        });

        let mut runner = TestRunner::new(config);

        let strategy = self
            .strategy
            .unwrap_or_else(|| ArgsStrategyBuilder::<Program>::new().build());
        let spec = self.spec.unwrap_or_else(|| Box::new(|_| Expect::Accept));
        let initial_transaction = self
            .initial_transaction
            .unwrap_or_else(|| FuzzTransaction::try_default().expect("Shouldn't fail. Please report a bug"));

        let result = runner.run(&strategy.no_shrink(), |(arguments, witness)| {
            Self::fuzz(&self.signer, &initial_transaction, &*spec, arguments, witness).map_err(
                |failure| match failure {
                    CaseFailure::Setup(error) => {
                        let reason = error.to_string();
                        *setup_error.lock().unwrap() = Some(error);

                        TestCaseError::fail(reason)
                    }
                    CaseFailure::Finding(reason) => TestCaseError::fail(reason),
                },
            )
        });

        if let Some(error) = setup_error.lock().unwrap().take() {
            panic!("The fuzz setup is broken, the contract was not judged: {error}");
        }

        match result {
            Ok(()) => {}
            Err(TestError::Fail(reason, (args, wit))) => {
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
        spec: &Spec<Program>,
        arguments: Arguments,
        witness: WitnessValues,
    ) -> Result<(), CaseFailure> {
        let network = signer.get_network();
        let (program, script) = Program::build_program(arguments.clone(), network);
        let program = program.as_ref().as_ref();

        let final_transaction = initial_tx
            .prepare_transaction(program, &script, &arguments, &witness)
            .map_err(|error| CaseFailure::Setup(SetupError::Transaction(error.to_string())))?;

        let (pst, signed_witnesses) = signer
            .sign_witnesses(&final_transaction)
            .map_err(|error| CaseFailure::Setup(SetupError::Signing(error.to_string())))?;

        let program_inputs: Vec<usize> = initial_tx
            .targets()
            .iter()
            .filter_map(|target| match target {
                ProgramTarget::Input(input_index) => Some(*input_index),
                ProgramTarget::Output(_) => None,
            })
            .collect();

        let case = FuzzCase::<Program>::new(&arguments, &witness, &pst, &program_inputs, signer)
            .map_err(CaseFailure::Setup)?;

        // A panicking spec is a bug in the test, not a finding against the contract
        let expect =
            panic::catch_unwind(AssertUnwindSafe(|| spec(&case))).map_err(|_| CaseFailure::Setup(SetupError::Spec))?;

        // Classify every program input before judging, so a setup error is never masked
        let outcomes = program_inputs
            .iter()
            .map(|&input_index| {
                let signed_witness = signed_witnesses.get(&input_index).unwrap_or(&witness);
                let result = program.finalize(&pst, signed_witness, input_index, network);

                Ok((input_index, Outcome::classify(result).map_err(CaseFailure::Setup)?))
            })
            .collect::<Result<Vec<_>, CaseFailure>>()?;

        let accepted = outcomes.iter().all(|(_, outcome)| outcome.is_accepted());

        let finding = match (expect, accepted) {
            (Expect::Reject, true) => "unsound: the contract accepted a spend the spec rejects",
            (Expect::Accept, false) => "liveness: the contract rejected a spend the spec accepts",
            _ => return Ok(()),
        };

        let inputs = outcomes
            .iter()
            .map(|(input_index, outcome)| format!("input {input_index} {outcome}"))
            .collect::<Vec<_>>()
            .join(", ");

        Err(CaseFailure::Finding(format!("{finding} ({inputs})")))
    }
}

/// Keeps setup errors out of the persisted failures.
#[derive(Clone, Debug)]
struct SkipSetupErrors {
    inner: Box<dyn FailurePersistence>,
    setup_error: Arc<Mutex<Option<SetupError>>>,
}

impl FailurePersistence for SkipSetupErrors {
    fn load_persisted_failures2(&self, source_file: Option<&'static str>) -> Vec<PersistedSeed> {
        self.inner.load_persisted_failures2(source_file)
    }

    fn save_persisted_failure2(
        &mut self,
        source_file: Option<&'static str>,
        seed: PersistedSeed,
        shrunken_value: &dyn Debug,
    ) {
        if self.setup_error.lock().unwrap().is_none() {
            self.inner.save_persisted_failure2(source_file, seed, shrunken_value);
        }
    }

    fn box_clone(&self) -> Box<dyn FailurePersistence> {
        Box::new(self.clone())
    }

    fn eq(&self, other: &dyn FailurePersistence) -> bool {
        other
            .as_any()
            .downcast_ref::<Self>()
            .is_some_and(|other| *self.inner == *other.inner)
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use proptest::strategy::Just;
    use proptest::test_runner::RngSeed;

    use smplx_sdk::global::Verbosity;
    use smplx_sdk::program::Program;
    use smplx_sdk::provider::{EsploraProvider, SimplicityNetwork};
    use smplx_sdk::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};

    use crate::config::{DEFAULT_TEST_MNEMONIC, EsploraConfig, FuzzConfig, TestConfig};
    use crate::context::{FuzzMode, TestContext};

    use smplx_sdk::program::{ProgramFactory, ProgramSchema};

    use crate::fuzz::Expect;

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

    impl ProgramSchema for DummyProgram {
        type Arguments = EmptyArgs;
        type Witness = EmptyArgs;

        fn argument_types() -> Vec<(simplicityhl::TemplateProgramWitness, simplicityhl::ResolvedType)> {
            Vec::new()
        }

        fn witness_types() -> Vec<(simplicityhl::TemplateProgramWitness, simplicityhl::ResolvedType)> {
            Vec::new()
        }

        fn literals() -> Vec<simplicityhl::num::U256> {
            Vec::new()
        }
    }

    #[derive(Clone, Debug)]
    struct EmptyArgs;

    impl TryFrom<&Arguments> for EmptyArgs {
        type Error = String;

        fn try_from(_: &Arguments) -> Result<Self, String> {
            Ok(Self)
        }
    }

    impl TryFrom<&WitnessValues> for EmptyArgs {
        type Error = String;

        fn try_from(_: &WitnessValues) -> Result<Self, String> {
            Ok(Self)
        }
    }

    fn build_test_engine(context: TestContext<FuzzMode>) -> SimplexFuzzEngine<DummyProgram> {
        let mut final_tx = FinalTransaction::new();
        final_tx.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);

        let fuzz_tx = FuzzTransaction::new(final_tx, [ProgramTarget::Input(0)]).unwrap();

        context
            .engine()
            .with_custom_strategy(Just((Arguments::default(), WitnessValues::default())))
            .with_custom_transaction(fuzz_tx)
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
                seed: Some(23),
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
        let network = SimplicityNetwork::Liquid;

        assert_eq!(*engine.signer.get_network(), network);
        assert_eq!(engine.signer.get_address().params, network.address_params());
        assert!(engine.signer.get_provider().is_err());
        assert_eq!(engine.config.cases, 17);
        assert_eq!(engine.config.rng_seed, RngSeed::Fixed(23));
        assert_eq!(engine.config.verbose, 2);
        assert_eq!(engine.config.test_name, Some(test_name));
        assert!(!engine.config.fork);
        assert_eq!(engine.config.max_shrink_iters, 0);
        assert_eq!(engine.config.source_file, Some(source_file));
    }

    #[test]
    fn fuzz_network_defaults_to_regtest() {
        let context = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz("crate::default_network_fuzz_test", file!())
            .unwrap();

        let network = SimplicityNetwork::default_regtest();

        assert_eq!(*context.get_network(), network);
        assert_eq!(*context.random_signer().get_network(), network);

        let engine = build_test_engine(context);

        assert_eq!(*engine.signer.get_network(), network);
    }

    #[test]
    fn builder_overrides_fuzz_configuration_and_signer() {
        let config = TestConfig {
            fuzz: Some(FuzzConfig {
                cases: Some(17),
                seed: Some(23),
                network: Some("Liquid".to_string()),
            }),
            ..Default::default()
        };

        let context = TestContext::from_config(config)
            .unwrap()
            .fuzz("crate::custom_fuzz_test", file!())
            .unwrap();

        let network = SimplicityNetwork::LiquidTestnet;
        let custom_signer = Signer::new(
            DEFAULT_TEST_MNEMONIC,
            Box::new(EsploraProvider::new("http://localhost:3001".to_string(), network)),
        );

        let public_key = custom_signer.get_schnorr_public_key();

        let mut custom_config = context.get_fuzz_config().clone();
        custom_config.cases = 31;
        custom_config.rng_seed = RngSeed::Fixed(37);

        let engine = build_test_engine(context)
            .with_custom_signer(custom_signer)
            .with_custom_config(custom_config);

        assert_eq!(*engine.signer.get_network(), network);
        assert_eq!(engine.signer.get_schnorr_public_key(), public_key);
        assert_eq!(engine.signer.get_provider().unwrap().get_network(), &network);

        assert_eq!(engine.config.cases, 31);
        assert_eq!(engine.config.rng_seed, RngSeed::Fixed(37));
    }

    #[test]
    fn engine_can_be_built_repeatedly_from_one_context() {
        let context = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz("crate::repeated_engine_fuzz_test", file!())
            .unwrap();

        let first = context.engine::<DummyProgram>();
        let second = context.engine::<DummyProgram>();

        let default_key = context.get_default_signer().get_schnorr_public_key();

        assert_eq!(first.signer.get_schnorr_public_key(), default_key);
        assert_eq!(second.signer.get_schnorr_public_key(), default_key);
        assert_eq!(first.signer.get_network(), context.get_network());
        assert_eq!(second.signer.get_network(), context.get_network());
    }

    #[test]
    #[should_panic(expected = "The fuzz setup is broken")]
    fn setup_errors_stop_the_run() {
        let context = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz("crate::setup_error_fuzz_test", file!())
            .unwrap();

        let transaction = FuzzTransaction::try_default()
            .unwrap()
            .with_post_hook(|_, _, _, _| Err(crate::error::FuzzError::NoProgramTargets));

        context
            .engine::<DummyProgram>()
            .with_custom_strategy(Just((Arguments::default(), WitnessValues::default())))
            .with_custom_transaction(transaction)
            .run();
    }

    /// An engine that does not persist failures, for tests that expect one.
    fn finding_engine(test_name: &'static str) -> SimplexFuzzEngine<DummyProgram> {
        let mut engine = TestContext::from_config(TestConfig::default())
            .unwrap()
            .fuzz(test_name, file!())
            .unwrap()
            .engine::<DummyProgram>()
            .with_custom_strategy(Just((Arguments::default(), WitnessValues::default())));

        engine.config.failure_persistence = None;

        engine
    }

    #[test]
    #[should_panic(expected = "unsound: the contract accepted a spend the spec rejects")]
    fn spec_rejecting_an_accepted_spend_is_unsound() {
        finding_engine("crate::unsound_fuzz_test")
            .spec(|_| Expect::Reject)
            .run();
    }

    #[test]
    #[should_panic(expected = "the spec panicked")]
    fn panicking_spec_is_a_setup_error() {
        finding_engine("crate::panicking_spec_fuzz_test")
            .spec(|_| panic!("spec bug"))
            .run();
    }
}
