use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
use simplex::simplicityhl::{Arguments, WitnessValues};
use simplex::{FuzzMode, TestContext};

use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::core::FuzzContext;
use simplex::fuzz::fuzz_transaction::{FuzzTransaction, ProgramTarget};
use simplex::fuzz::proptest::strategy::Just;
use simplex::fuzz::{FuzzError, ProgramCheck, ProgramExecResult};
use simplex::provider::SimplicityNetwork;
use simplex::signer::Signer;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};
use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum Expect {
    Ok,
    Failure,
}

struct FailureProgramCheck {
    expect: Expect,
}

struct SuccessfulProgramCheck;

impl ProgramCheck<FailureTestProgram, FailureTestArguments, FailureTestWitness> for FailureProgramCheck {
    fn call(
        &self,
        _ctx: &FuzzContext,
        _tx: &PartiallySignedTransaction,
        _arguments: &Arguments,
        _witness: &WitnessValues,
        _input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String> {
        let args = FailureTestArguments::from_arguments(_arguments)?;
        let witness = FailureTestWitness::from_witness(_witness)?;

        if (args.failure_value == witness.cmp_value || program_exec_result.is_err()) && self.expect == Expect::Ok {
            return Err(format!(
                "Failed contract, failure_value == cmp_value , {program_exec_result:?}"
            ));
        }

        Ok(())
    }
}

impl ProgramCheck<P2pkProgram, P2pkArguments, P2pkWitness> for SuccessfulProgramCheck {
    fn call(
        &self,
        _ctx: &FuzzContext,
        _tx: &PartiallySignedTransaction,
        _arguments: &Arguments,
        witness: &WitnessValues,
        _input_index: usize,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String> {
        let signed_witness = P2pkWitness::from_witness(witness)?;

        if signed_witness == P2pkWitness::default() {
            return Err("fuzz check received the original unsigned witness".to_string());
        }

        program_exec_result
            .map(|_| ())
            .map_err(|error| format!("signed program failed: {error}"))
    }
}

fn signed_fuzz_transaction(arguments: &P2pkArguments, witness: &P2pkWitness) -> Result<FuzzTransaction, FuzzError> {
    const FUZZ_PROGRAM_TARGET: ProgramTarget = ProgramTarget::Input(0);

    let program = P2pkProgram::new(arguments.clone());
    let mut transaction = FinalTransaction::new();

    transaction.add_program_input(
        PartialInput::new(UTXO::default()),
        ProgramInput::new(Box::new(program.as_ref().clone()), witness),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    FuzzTransaction::new(transaction, [FUZZ_PROGRAM_TARGET])
}

#[simplex::fuzz]
fn test_failure_ignoring_in_fuzzing(mut test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    test_context.set_network(SimplicityNetwork::default_regtest())?;

    let strategy = ArgsStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
    let initial_transaction = FuzzTransaction::try_default()?;

    let runner = test_context.build(strategy, initial_transaction);

    runner.run_with_check(FailureProgramCheck {
        expect: Expect::Failure,
    });

    Ok(())
}

#[should_panic(expected = "Program failed with these arguments")]
#[simplex::fuzz]
fn test_panic_after_fuzzing(mut test_context: TestContext<FuzzMode>) {
    test_context.set_network(SimplicityNetwork::default_regtest()).unwrap();

    let strategy = ArgsStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
    let initial_transaction = FuzzTransaction::try_default().unwrap();
    let runner = test_context.build(strategy, initial_transaction);

    runner.run_with_check(FailureProgramCheck { expect: Expect::Ok });
}

#[simplex::fuzz]
fn test_signed_witness_with_program_checks(mut test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    const TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

    let network = SimplicityNetwork::default_regtest();
    let signer = Signer::from_mnemonic(TEST_MNEMONIC, network);
    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    test_context.set_network(network)?;
    test_context.set_custom_signer(signer);

    let witness = P2pkWitness::default();
    let strategy = Just(((&arguments).into(), (&witness).into()));

    let initial_transaction = signed_fuzz_transaction(&arguments, &witness)?;
    let runner = test_context.build(strategy, initial_transaction);

    runner.run_with_check(SuccessfulProgramCheck);

    Ok(())
}
