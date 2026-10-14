use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, Scenario};
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[simplex::fuzz]
fn failure_test_matches_its_spec(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let mut scenario = Scenario::default();
    scenario.add::<FailureTestProgram>("failure").spends(0);

    test_context
        .engine(scenario)
        .spec(|case| {
            let failure = case.program::<FailureTestProgram>("failure");

            if failure.arguments().failure_value == failure.witness().cmp_value {
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
    let mut scenario = Scenario::default();
    scenario.add::<FailureTestProgram>("failure").spends(0);

    test_context.engine(scenario).run();
}

#[simplex::fuzz]
fn signed_witness_is_accepted(test_context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let signer = test_context.get_default_signer();

    let public_key = signer.get_schnorr_public_key().serialize();

    let mut transaction = FinalTransaction::new();
    transaction.add_program_input(
        PartialInput::new(UTXO::default()),
        ProgramInput::new(
            Box::new(P2pkProgram::new(P2pkArguments { public_key }).as_ref().clone()),
            P2pkWitness::default(),
        ),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    let mut scenario = Scenario::new(transaction);
    scenario
        .add::<P2pkProgram>("p2pk")
        .spends(0)
        .bind_arguments(move |arguments, _| arguments.public_key = public_key);

    test_context.engine(scenario).run();

    Ok(())
}
