use std::any::Any;
use std::cell::RefCell;
use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use proptest::prelude::TestCaseError;
use proptest::strategy::Strategy;
use proptest::test_runner::{Config as ProptestConfig, FailurePersistence, PersistedSeed, TestError, TestRunner};

use simplicityhl::{Arguments, WitnessValues};

use smplx_sdk::signer::Signer;

use crate::fuzz::outcome::{Outcome, SetupError};
use crate::fuzz::scenario::Scenario;
use crate::fuzz::smart_check::AuthCheck;
use crate::fuzz::spec::{Expect, FuzzCase};
use crate::fuzz::summary::RunSummary;

type Spec = dyn Fn(&FuzzCase<'_>) -> Expect;

enum CaseFailure {
    Setup(SetupError),
    Finding(String),
}

pub struct SimplexFuzzEngine {
    pub config: ProptestConfig,
    pub signer: Signer,
    pub scenario: Scenario,
    pub spec: Box<Spec>,
    pub allow_unspendable: bool,
    pub check_auth: bool,
}

impl SimplexFuzzEngine {
    pub fn new(config: ProptestConfig, signer: Signer, scenario: Scenario) -> SimplexFuzzEngine {
        SimplexFuzzEngine {
            config,
            signer,
            scenario,
            spec: Box::new(|_| Expect::Accept),
            allow_unspendable: false,
            check_auth: true,
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

    /// Sets what should happen to each spend. Without a spec, every spend is expected to be accepted.
    #[must_use]
    pub fn spec(mut self, spec: impl Fn(&FuzzCase<'_>) -> Expect + 'static) -> Self {
        self.spec = Box::new(spec);

        self
    }

    /// Lets a run pass without any accepted case, for tests where no spend should ever be accepted.
    #[must_use]
    pub fn allow_unspendable(mut self) -> Self {
        self.allow_unspendable = true;

        self
    }

    /// Skips checking that accepted spends depend on their signatures.
    #[must_use]
    pub fn without_auth_check(mut self) -> Self {
        self.check_auth = false;

        self
    }

    pub fn run(self) {
        if let Err(error) = self.scenario.validate() {
            panic!("The fuzz setup is broken, the contract was not tested: {error}");
        }

        let setup_error = Arc::new(Mutex::new(None));
        let summary = RefCell::new(RunSummary::new(
            self.config.test_name.unwrap_or("simplex_fuzz"),
            self.config.rng_seed,
            self.scenario.layout(),
            self.check_auth,
        ));

        let mut config = self.config.clone();
        config.failure_persistence = config.failure_persistence.map(|inner| {
            Box::new(SkipSetupErrors {
                inner,
                setup_error: setup_error.clone(),
            }) as Box<dyn FailurePersistence>
        });

        let mut runner = TestRunner::new(config);

        let result = runner.run(&self.scenario.strategy().no_shrink(), |values| {
            self.fuzz(&summary, values).map_err(|failure| match failure {
                CaseFailure::Setup(error) => {
                    let reason = error.to_string();
                    *setup_error.lock().unwrap() = Some(error);

                    TestCaseError::fail(reason)
                }
                CaseFailure::Finding(reason) => TestCaseError::fail(reason),
            })
        });

        let summary = summary.into_inner();
        eprintln!("{summary}");

        if let Some(error) = setup_error.lock().unwrap().take() {
            panic!("The fuzz setup is broken, the contract was not tested: {error}");
        }

        match result {
            Ok(()) => {
                if let Some(reason) = summary.is_run_empty(self.allow_unspendable) {
                    panic!("{reason}");
                }
            }
            Err(TestError::Fail(reason, values)) => {
                let values = self
                    .scenario
                    .program_scenario_names()
                    .zip(values)
                    .map(|(name, (arguments, witness))| {
                        format!("`{name}` arguments: {arguments} and witness: {witness}")
                    })
                    .collect::<Vec<_>>()
                    .join("; ");

                panic!("Case failed with {values}, reason: `{reason}`");
            }
            Err(err) => {
                panic!("Fuzzing aborted: {err}");
            }
        }
    }

    /// Extracted helper that performs exactly one isolated test run.
    fn fuzz(&self, summary: &RefCell<RunSummary>, values: Vec<(Arguments, WitnessValues)>) -> Result<(), CaseFailure> {
        let network = self.signer.get_network();

        let (programs, transaction) = self.scenario.build(values, network).map_err(CaseFailure::Setup)?;

        let (pst, signed_witnesses) = self
            .signer
            .sign_witnesses(&transaction)
            .map_err(|error| CaseFailure::Setup(SetupError::Signing(error.to_string())))?;

        let case = FuzzCase::new(&self.scenario, &programs, &transaction, &pst, &self.signer);

        // A panicking spec is a bug in the test, not a finding against the contract
        let expect = panic::catch_unwind(AssertUnwindSafe(|| (self.spec)(&case)))
            .map_err(|_| CaseFailure::Setup(SetupError::Spec))?;

        let program_inputs: Vec<usize> = transaction
            .inputs()
            .iter()
            .enumerate()
            .filter_map(|(index, input)| input.program_input.as_ref().map(|_| index))
            .collect();

        let outcomes = program_inputs
            .iter()
            .map(|&index| {
                let program = &transaction.inputs()[index]
                    .program_input
                    .as_ref()
                    .expect("filtered to program inputs")
                    .program;
                let result = program.finalize(&pst, &signed_witnesses[&index], index, network);

                Ok((index, Outcome::classify(result).map_err(CaseFailure::Setup)?))
            })
            .collect::<Result<Vec<_>, CaseFailure>>()?;

        let rejection = outcomes.iter().find(|(_, outcome)| !outcome.is_accepted());
        let verdict = rejection.map_or(Outcome::Accepted, |(_, outcome)| *outcome);

        summary.borrow_mut().record(
            expect,
            verdict,
            rejection.map(|(index, _)| {
                self.scenario.program_scenario_by_input(*index).map_or_else(
                    || format!("input {index}"),
                    |program_scenario| program_scenario.name.clone(),
                )
            }),
        );

        let finding = match (expect, verdict.is_accepted()) {
            (Expect::Reject, true) => Some("unsound: the contract accepted a spend the spec rejects"),
            (Expect::Accept, false) => Some("liveness: the contract rejected a spend the spec accepts"),
            _ => None,
        };

        if let Some(finding) = finding {
            let inputs = outcomes
                .iter()
                .map(
                    |(index, outcome)| match self.scenario.program_scenario_by_input(*index) {
                        Some(program_scenario) => format!("input {index} `{}` {outcome}", program_scenario.name),
                        None => format!("input {index} {outcome}"),
                    },
                )
                .collect::<Vec<_>>()
                .join(", ");

            return Err(CaseFailure::Finding(format!("{finding} ({inputs})")));
        }

        if self.check_auth
            && verdict.is_accepted()
            && let Some(check) = AuthCheck::new(&self.signer, &transaction, &program_inputs)
        {
            let bypasses = check.run().map_err(CaseFailure::Setup)?;
            summary.borrow_mut().record_auth_check();

            if !bypasses.is_empty() {
                return Err(CaseFailure::Finding(format!(
                    "authorization bypass: {}",
                    bypasses.join("; ")
                )));
            }
        }

        Ok(())
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
    use proptest::test_runner::RngSeed;

    use smplx_sdk::global::Verbosity;
    use smplx_sdk::provider::{EsploraProvider, SimplicityNetwork};

    use crate::config::{DEFAULT_TEST_MNEMONIC, EsploraConfig, FuzzConfig, TestConfig};
    use crate::context::TestContext;

    use super::*;

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

        let engine = context.engine(Scenario::default());
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

        let engine = context.engine(Scenario::default());

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

        let engine = context
            .engine(Scenario::default())
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

        let first = context.engine(Scenario::default());
        let second = context.engine(Scenario::default());

        let default_key = context.get_default_signer().get_schnorr_public_key();

        assert_eq!(first.signer.get_schnorr_public_key(), default_key);
        assert_eq!(second.signer.get_schnorr_public_key(), default_key);
        assert_eq!(first.signer.get_network(), context.get_network());
        assert_eq!(second.signer.get_network(), context.get_network());
    }
}
