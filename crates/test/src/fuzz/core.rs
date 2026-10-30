use std::any::Any;
use std::cell::RefCell;
use std::fmt::Debug;
use std::panic::{self, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use proptest::prelude::TestCaseError;
use proptest::strategy::Strategy;
use proptest::test_runner::{Config as ProptestConfig, FailurePersistence, PersistedSeed, TestError, TestRunner};

use smplx_sdk::signer::Signer;

use crate::fuzz::case::{Expect, FuzzCase};
use crate::fuzz::execution::SignedTransaction;
use crate::fuzz::mutations::Mutations;
use crate::fuzz::outcome::{Outcome, SetupError};
use crate::fuzz::requirement::RequiredValue;
use crate::fuzz::scenario::Scenario;
use crate::fuzz::scenario_strategy::CaseValues;
use crate::fuzz::smart_check::{AuthCheck, LockCheck};
use crate::fuzz::summary::RunSummary;

type Spec = dyn Fn(&FuzzCase<'_>) -> Expect;

/// The smart checks' attacker. Fixed, so a replayed case meets the same attacker.
pub(crate) const ATTACKER_MNEMONIC: &str =
    "legal winner thank year wave sausage worth useful legal winner thank yellow";

enum CaseFailure {
    Setup(SetupError),
    Finding(String),
}

pub struct SimplexFuzzEngine {
    pub config: ProptestConfig,
    pub signer: Signer,
    pub scenario: Scenario,
    pub spec: Box<Spec>,
    pub mutations: Option<Mutations>,
    pub allow_unspendable: bool,
    pub check_lock: bool,
    pub check_auth: bool,
}

impl SimplexFuzzEngine {
    pub fn new(config: ProptestConfig, signer: Signer, scenario: Scenario) -> SimplexFuzzEngine {
        SimplexFuzzEngine {
            config,
            signer,
            scenario,
            spec: Box::new(|_| Expect::Accept),
            mutations: None,
            allow_unspendable: false,
            check_lock: true,
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

    /// Mutates the transaction in half the cases. The spec judges the mutated transaction.
    #[must_use]
    pub fn mutations(mut self, mutations: Mutations) -> Self {
        self.mutations = Some(mutations);

        self
    }

    /// Lets a run pass without any accepted case, for tests where no spend should ever be accepted.
    #[must_use]
    pub fn allow_unspendable(mut self) -> Self {
        self.allow_unspendable = true;

        self
    }

    /// Skips checking that accepted spends cannot redirect the outputs they lock.
    #[must_use]
    pub fn without_lock_check(mut self) -> Self {
        self.check_lock = false;

        self
    }

    /// Skips checking that accepted spends depend on their signatures.
    #[must_use]
    pub fn without_auth_check(mut self) -> Self {
        self.check_auth = false;

        self
    }

    pub fn run(self) {
        let strategy = match self
            .scenario
            .validate()
            .and_then(|()| self.scenario.combined_strategy(self.mutations.as_ref()))
        {
            Ok(strategy) => strategy,
            Err(error) => panic!("The fuzz setup is broken, the contract was not tested: {error}"),
        };

        let attacker = Signer::from_mnemonic(ATTACKER_MNEMONIC, *self.signer.get_network());
        assert!(
            attacker.get_schnorr_public_key() != self.signer.get_schnorr_public_key(),
            "The fuzz setup is broken, the contract was not tested: the signer uses the smart checks attacker mnemonic"
        );

        let setup_error = Arc::new(Mutex::new(None));
        let summary = RefCell::new(RunSummary::new(
            self.config.test_name.unwrap_or("simplex_fuzz"),
            self.config.rng_seed,
            self.scenario.layout(),
            self.check_lock,
            self.check_auth,
            self.mutations.is_some(),
        ));

        let mut config = self.config.clone();
        config.failure_persistence = config.failure_persistence.map(|inner| {
            Box::new(SkipSetupErrors {
                inner,
                setup_error: setup_error.clone(),
            }) as Box<dyn FailurePersistence>
        });

        let mut runner = TestRunner::new(config);

        let result = runner.run(&strategy.no_shrink(), |values| {
            self.fuzz(&summary, &attacker, values).map_err(|failure| match failure {
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
            Err(TestError::Fail(reason, _)) => {
                panic!("{reason}");
            }
            Err(err) => {
                panic!("Fuzzing aborted: {err}");
            }
        }
    }

    /// Builds one case, asks the spec, runs it and reports what disagrees.
    fn fuzz(&self, summary: &RefCell<RunSummary>, attacker: &Signer, values: CaseValues) -> Result<(), CaseFailure> {
        let mutations = values.change.mutations().to_vec();
        let drew_violation = values.change.violation().is_some();

        let (programs, transaction, violated) =
            self.scenario.build(values, &self.signer).map_err(CaseFailure::Setup)?;

        if drew_violation && violated.is_none() {
            summary.borrow_mut().record_unviolated();
        }
        let case = FuzzCase::new(&self.scenario, &programs, &transaction, &mutations, &self.signer);

        let failure =
            |reason: String| CaseFailure::Finding(format!("Case failed with {}, reason: `{reason}`", case.describe()));

        // A generated case that violates a requirement must be rejected, so the spec is not asked.
        let expect = match violated {
            Some(_) => None,
            None => Some(self.ask_spec(&case)?),
        };

        let outcomes = SignedTransaction::new(&self.signer, &transaction)
            .and_then(|signed| signed.run_all())
            .map_err(CaseFailure::Setup)?;
        let rejection = outcomes.iter().find(|(_, outcome)| !outcome.is_accepted());
        let verdict = rejection.map_or(Outcome::Accepted, |(_, outcome)| *outcome);

        summary.borrow_mut().record(
            expect,
            verdict,
            rejection.map(|(index, _)| {
                case.spending_program(*index)
                    .map_or_else(|| format!("input {index}"), ToString::to_string)
            }),
        );
        summary.borrow_mut().record_mutations(&mutations);

        if let Some(finding) = Self::finding(expect, verdict, violated.as_ref()) {
            return Err(failure(format!(
                "{finding} ({})",
                Self::describe_outcomes(&case, &outcomes)
            )));
        }

        if verdict.is_accepted()
            && let Some(bypass) = self
                .do_smart_checks(summary, attacker, &case)
                .map_err(CaseFailure::Setup)?
        {
            return Err(failure(bypass));
        }

        Ok(())
    }

    /// A panicking spec is a bug in the test, not a finding against the contract.
    fn ask_spec(&self, case: &FuzzCase<'_>) -> Result<Expect, CaseFailure> {
        panic::catch_unwind(AssertUnwindSafe(|| (self.spec)(case))).map_err(|_| CaseFailure::Setup(SetupError::Spec))
    }

    /// What the verdict means against the expectation, `None` when they agree.
    /// `expect` is `None` for a case that violates a requirement.
    fn finding(expect: Option<Expect>, verdict: Outcome, violated: Option<&RequiredValue<'_>>) -> Option<String> {
        match (expect, verdict.is_accepted(), violated) {
            (None, true, Some(violated)) => Some(format!(
                "unsound: the contract accepted a spend that violates a requirement, {violated}"
            )),
            (Some(Expect::Reject), true, _) => {
                Some("unsound: the contract accepted a spend the spec rejects".to_string())
            }
            (Some(Expect::Accept), false, _) => {
                Some("liveness: the contract rejected a spend the spec accepts".to_string())
            }
            _ => None,
        }
    }

    /// Every program input with its outcome, e.g. "input 0 `vault` accepted, input 1 rejected (assertion)".
    fn describe_outcomes(case: &FuzzCase<'_>, outcomes: &[(usize, Outcome)]) -> String {
        outcomes
            .iter()
            .map(|(index, outcome)| match case.spending_program(*index) {
                Some(name) => format!("input {index} `{name}` {outcome}"),
                None => format!("input {index} {outcome}"),
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Runs the smart checks on an accepted case and returns the first bypass they find.
    fn do_smart_checks(
        &self,
        summary: &RefCell<RunSummary>,
        attacker: &Signer,
        case: &FuzzCase<'_>,
    ) -> Result<Option<String>, SetupError> {
        let transaction = case.transaction();

        if self.check_lock
            && let Some(check) = LockCheck::new(&self.signer, attacker, transaction, case.locked_outputs())
        {
            let bypasses = check.run()?;
            summary.borrow_mut().record_lock_check();

            if !bypasses.is_empty() {
                return Ok(Some(format!("lock bypass: {}", bypasses.join("; "))));
            }
        }

        if self.check_auth
            && let Some(check) = AuthCheck::new(&self.signer, attacker, transaction)
        {
            let bypasses = check.run()?;
            summary.borrow_mut().record_auth_check();

            if !bypasses.is_empty() {
                return Ok(Some(format!("authorization bypass: {}", bypasses.join("; "))));
            }
        }

        Ok(None)
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
