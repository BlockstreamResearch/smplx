use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
use simplex::simplicityhl::{Arguments, WitnessValues};
use simplex::{FuzzMode, TestContext};

use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::fuzz_transaction::{FuzzTransaction, ProgramTarget};
use simplex::fuzz::proptest::strategy::Just;
use simplex::fuzz::{FuzzError, ProgramCheck, ProgramExecResult};
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
        _signer: &Signer,
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
        _signer: &Signer,
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

fn initial_fuzz_transaction(arguments: &P2pkArguments, witness: &P2pkWitness) -> Result<FuzzTransaction, FuzzError> {
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
fn test_failure_ignoring_in_fuzzing(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let strategy = ArgsStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
    let initial_transaction = FuzzTransaction::try_default()?;

    test_context
        .engine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>()
        .with_custom_strategy(strategy)
        .with_custom_transaction(initial_transaction)
        .with_custom_check(FailureProgramCheck {
            expect: Expect::Failure,
        })
        .run();

    Ok(())
}

#[should_panic(expected = "Program failed with these arguments")]
#[simplex::fuzz]
fn test_panic_after_fuzzing(test_context: TestContext<FuzzMode>) {
    let strategy = ArgsStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
    let transaction = FuzzTransaction::try_default().unwrap();

    test_context
        .engine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>()
        .with_custom_strategy(strategy)
        .with_custom_transaction(transaction)
        .with_custom_check(FailureProgramCheck { expect: Expect::Ok })
        .run();
}

#[simplex::fuzz]
fn test_signed_witness_with_program_checks(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    const TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

    let signer = Signer::from_mnemonic(TEST_MNEMONIC, *test_context.get_network());

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let witness = P2pkWitness::default();
    let strategy = Just(((&arguments).into(), (&witness).into()));

    let transaction = initial_fuzz_transaction(&arguments, &witness)?;

    test_context
        .engine::<P2pkProgram, P2pkArguments, P2pkWitness>()
        .with_custom_signer(signer)
        .with_custom_strategy(strategy)
        .with_custom_transaction(transaction)
        .with_custom_check(SuccessfulProgramCheck)
        .run();

    Ok(())
}
