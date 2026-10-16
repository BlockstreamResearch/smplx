use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, Scenario};
use simplex::simplicityhl::WitnessValues;
use simplex::simplicityhl::elements::{OutPoint, Script, Txid};
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::counter::CounterProgram;
use simplex_fixtures::artifacts::dummy_accept::DummyAcceptProgram;
use simplex_fixtures::artifacts::dummy_accept::derived_dummy_accept::DummyAcceptArguments;
use simplex_fixtures::artifacts::dummy_panic::DummyPanicProgram;
use simplex_fixtures::artifacts::requires_input::RequiresInputProgram;
use simplex_fixtures::artifacts::tapleaf_check::TapleafCheckProgram;
use simplex_fixtures::artifacts::unvault::UnvaultProgram;
use simplex_fixtures::artifacts::vault::VaultProgram;

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

fn one_input_one_output(test_context: &TestContext<FuzzMode>) -> FinalTransaction {
    let mut transaction = FinalTransaction::new();
    transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);
    transaction.add_output(PartialOutput::new(
        Script::new(),
        1,
        test_context.get_network().policy_asset(),
    ));

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

#[simplex::fuzz]
fn vault_bound_to_its_unvault_passes(test_context: TestContext<FuzzMode>) {
    let mut scenario = Scenario::new(one_input_one_output(&test_context));

    scenario.add::<UnvaultProgram>("unvault").locks(0).random_strategy();
    scenario
        .add::<VaultProgram>("vault")
        .spends(0)
        .bind_arguments(|arguments, case| {
            arguments.unvault_script_hash = case.program::<UnvaultProgram>("unvault").script_hash();
        });

    test_context.engine(scenario).run();
}

#[simplex::fuzz]
fn program_bound_to_a_fixed_input_passes(test_context: TestContext<FuzzMode>) {
    let fixed = DummyAcceptProgram::new(DummyAcceptArguments::default());

    let mut fixed_utxo = UTXO {
        outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), 1),
        ..UTXO::default()
    };
    fixed_utxo.txout.script_pubkey = fixed.get_script_pubkey(test_context.get_network());

    let mut transaction = FinalTransaction::new();
    transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);
    transaction.add_program_input(
        PartialInput::new(fixed_utxo),
        ProgramInput::new(Box::new(fixed.as_ref().clone()), WitnessValues::default()),
        RequiredSignature::None,
    );

    let mut scenario = Scenario::new(transaction);

    scenario
        .add::<RequiresInputProgram>("requires_input")
        .spends(0)
        .bind_arguments(|arguments, case| {
            arguments.required_script_hash = case.input_script_hash(1);
        });

    test_context.engine(scenario).run();
}

#[simplex::fuzz]
fn tapleaf_check_bound_to_its_own_tapleaf_passes(test_context: TestContext<FuzzMode>) {
    let mut scenario = Scenario::default();

    scenario
        .add::<TapleafCheckProgram>("tapleaf_check")
        .spends(0)
        .bind_witness(|witness, case| {
            witness.program_tapleaf_hash = case
                .program::<TapleafCheckProgram>("tapleaf_check")
                .program()
                .get_tapleaf_hash();
        });

    test_context.engine(scenario).run();
}

/// The counter adds `STEP` to the lowest 64 bits of its state. `extra` makes the modelled transition wrong.
fn counter(test_context: &TestContext<FuzzMode>, extra: u64) {
    let mut scenario = Scenario::new(one_input_one_output(test_context));

    scenario
        .add::<CounterProgram>("counter")
        .storage_capacity(1)
        .spends(0)
        .bind_witness(|witness, case| {
            witness.state = case.program::<CounterProgram>("counter").storage()[0];
        })
        .recursive(0, move |counter| {
            let mut state = counter.storage()[0];
            let low = u64::from_be_bytes(state[24..].try_into().unwrap())
                .wrapping_add(counter.witness().step)
                .wrapping_add(extra);

            state[24..].copy_from_slice(&low.to_be_bytes());

            vec![state]
        });

    test_context
        .engine(scenario)
        .spec(|case| {
            let counter = case.program::<CounterProgram>("counter");
            let low = u64::from_be_bytes(counter.storage()[0][24..].try_into().unwrap());
            let step = counter.witness().step;

            if step <= counter.arguments().max_step && low.checked_add(step).is_some() {
                Expect::Accept
            } else {
                Expect::Reject
            }
        })
        .run();
}

#[simplex::fuzz]
fn counter_with_its_transition_passes(test_context: TestContext<FuzzMode>) {
    counter(&test_context, 0);
}

#[should_panic(expected = "liveness: the contract rejected a spend the spec accepts")]
#[simplex::fuzz]
fn counter_with_an_off_by_one_transition_is_live(test_context: TestContext<FuzzMode>) {
    counter(&test_context, 1);
}

#[should_panic(expected = "lock bypass: output 0 (locked to `unvault`) can be redirected")]
#[simplex::fuzz]
fn vault_that_ignores_its_output_is_redirected(test_context: TestContext<FuzzMode>) {
    let mut scenario = Scenario::new(one_input_one_output(&test_context));

    scenario.add::<UnvaultProgram>("unvault").locks(0);
    scenario.add::<DummyAcceptProgram>("leaky_vault").spends(0);

    test_context.engine(scenario).run();
}

#[simplex::fuzz]
fn unchecked_lock_may_be_redirected(test_context: TestContext<FuzzMode>) {
    let mut scenario = Scenario::new(one_input_one_output(&test_context));

    scenario.add::<UnvaultProgram>("unvault").locks_unchecked(0);
    scenario.add::<DummyAcceptProgram>("key_path").spends(0);

    test_context.engine(scenario).run();
}
