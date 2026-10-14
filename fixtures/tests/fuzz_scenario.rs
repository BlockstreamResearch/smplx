use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, Scenario};
use simplex::simplicityhl::elements::{OutPoint, Txid};
use simplex::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::dummy_accept::DummyAcceptProgram;
use simplex_fixtures::artifacts::dummy_panic::DummyPanicProgram;

fn two_inputs() -> FinalTransaction {
    let mut transaction = FinalTransaction::new();

    for vout in 0..2 {
        let utxo = UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), vout),
            ..UTXO::default()
        };

        transaction.add_input(PartialInput::new(utxo), RequiredSignature::None);
    }

    transaction
}

fn accepting() -> Scenario {
    let mut scenario = Scenario::default();
    scenario.add::<DummyAcceptProgram>("accepting").spends(0);

    scenario
}

fn rejecting() -> Scenario {
    let mut scenario = Scenario::default();
    scenario.add::<DummyPanicProgram>("rejecting").spends(0);

    scenario
}

#[should_panic(expected = "liveness: the contract rejected a spend the spec accepts \
                           (input 0 `accepting` accepted, input 1 `rejecting` rejected (assertion))")]
#[simplex::fuzz]
fn co_spent_program_that_rejects_is_named(test_context: TestContext<FuzzMode>) {
    let mut scenario = Scenario::new(two_inputs());

    scenario.add::<DummyAcceptProgram>("accepting").spends(0);
    scenario.add::<DummyPanicProgram>("rejecting").spends(1);

    test_context.engine(scenario).run();
}

#[should_panic(expected = "unsound: the contract accepted a spend the spec rejects")]
#[simplex::fuzz]
fn spec_rejecting_an_accepted_spend_is_unsound(test_context: TestContext<FuzzMode>) {
    test_context.engine(accepting()).spec(|_| Expect::Reject).run();
}

#[should_panic(expected = "the spec panicked")]
#[simplex::fuzz]
fn panicking_spec_is_a_setup_error(test_context: TestContext<FuzzMode>) {
    test_context.engine(accepting()).spec(|_| panic!("spec bug")).run();
}

#[should_panic(expected = "The spec made no prediction")]
#[simplex::fuzz]
fn spec_without_predictions_fails(test_context: TestContext<FuzzMode>) {
    test_context.engine(accepting()).spec(|_| Expect::Any).run();
}

#[should_panic(expected = "No case was accepted")]
#[simplex::fuzz]
fn run_without_acceptance_fails(test_context: TestContext<FuzzMode>) {
    test_context.engine(rejecting()).spec(|_| Expect::Reject).run();
}

#[simplex::fuzz]
fn allow_unspendable_lets_a_rejecting_run_pass(test_context: TestContext<FuzzMode>) {
    test_context
        .engine(rejecting())
        .spec(|_| Expect::Reject)
        .allow_unspendable()
        .run();
}
