use simplex::{FuzzMode, TestContext};

use simplex::fuzz::fuzz_transaction::{FuzzTransaction, ProgramTarget};
use simplex::fuzz::proptest::strategy::Just;
use simplex::fuzz::{Expect, FuzzError};
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

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
fn failure_test_matches_its_spec(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    test_context
        .engine::<FailureTestProgram>()
        .spec(|case| {
            if case.arguments().failure_value == case.witness().cmp_value {
                Expect::Reject
            } else {
                Expect::Accept
            }
        })
        .run();

    Ok(())
}

#[should_panic(expected = "liveness: the contract rejected a spend the spec accepts")]
#[simplex::fuzz]
fn default_spec_reports_liveness(test_context: TestContext<FuzzMode>) {
    test_context.engine::<FailureTestProgram>().run();
}

#[simplex::fuzz]
fn signed_witness_is_accepted(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let signer = test_context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let witness = P2pkWitness::default();
    let strategy = Just(((&arguments).into(), (&witness).into()));

    let transaction = initial_fuzz_transaction(&arguments, &witness)?;

    test_context
        .engine::<P2pkProgram>()
        .with_custom_strategy(strategy)
        .with_custom_transaction(transaction)
        .run();

    Ok(())
}
