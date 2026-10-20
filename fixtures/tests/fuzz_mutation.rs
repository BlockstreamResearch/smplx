use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, FuzzCase, Mutation, Mutations, Scenario};
use simplex::simplicityhl::elements::{AssetId, LockTime, OutPoint, Script, Sequence};
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, UTXO};

use simplex_fixtures::artifacts::conserving_vault::ConservingVaultProgram;
use simplex_fixtures::artifacts::dummy_accept::DummyAcceptProgram;
use simplex_fixtures::artifacts::paired_output::PairedOutputProgram;
use simplex_fixtures::artifacts::script_auth::ScriptAuthProgram;
use simplex_fixtures::artifacts::timelock::TimelockProgram;
use simplex_fixtures::artifacts::unvault::UnvaultProgram;
use simplex_fixtures::artifacts::vault::VaultProgram;

/// Input 0 holds 1000 of the vault's coins, output 0 pays them to the unvault and output 1 is the fee.
fn vault_scenario(fee: u64) -> Scenario {
    let asset = AssetId::from_byte_array([1; 32]);

    let mut transaction = FinalTransaction::new();
    transaction.add_input(
        PartialInput::new(UTXO::explicit(OutPoint::default(), Script::new(), 1000, asset)),
        None,
    );
    transaction.add_output(PartialOutput::new(Script::new(), 1000 - fee, asset));
    transaction.add_output(PartialOutput::new(Script::new(), 0, asset));

    let mut scenario = Scenario::new(transaction);
    scenario.add::<UnvaultProgram>("unvault").locks(0).random_strategy();

    scenario
}

/// Output 0 pays the vault's whole input, in its asset, to the unvault.
fn pays_unvault(case: &FuzzCase<'_>) -> Expect {
    let unvault = case.program::<UnvaultProgram>("unvault");
    let vault = case.input_index(0);

    if case.output_script_hash(0) == unvault.script_hash()
        && case.output_amount(0) == case.input_amount(vault)
        && case.output_asset(0) == case.input_asset(vault)
    {
        Expect::Accept
    } else {
        Expect::Reject
    }
}

#[should_panic(expected = "unsound: the contract accepted a spend the spec rejects")]
#[simplex::fuzz]
fn vault_that_ignores_the_amount_is_unsound(test_context: TestContext<FuzzMode>) {
    script_only_vault(&test_context, Mutations::none().amounts());
}

#[should_panic(expected = "unsound: the contract accepted a spend the spec rejects")]
#[simplex::fuzz]
fn vault_that_ignores_the_asset_is_unsound(test_context: TestContext<FuzzMode>) {
    script_only_vault(&test_context, Mutations::none().assets());
}

/// The fixture vault checks only the script of output 0.
fn script_only_vault(test_context: &TestContext<FuzzMode>, mutations: Mutations) {
    let mut scenario = vault_scenario(0);

    scenario
        .add::<VaultProgram>("vault")
        .spends(0)
        .bind_arguments(|arguments, case| {
            arguments.unvault_script_hash = case.program::<UnvaultProgram>("unvault").script_hash();
        });

    test_context
        .engine(scenario)
        .mutations(mutations)
        .spec(pays_unvault)
        .run();
}

#[simplex::fuzz]
fn conserving_vault_passes(test_context: TestContext<FuzzMode>) {
    let mut scenario = vault_scenario(0);

    scenario
        .add::<ConservingVaultProgram>("vault")
        .spends(0)
        .bind_arguments(|arguments, case| {
            arguments.unvault_script_hash = case.program::<UnvaultProgram>("unvault").script_hash();
        });

    test_context
        .engine(scenario)
        .mutations(Mutations::default().layout())
        .spec(pays_unvault)
        .run();
}

#[should_panic(expected = "mutations need a balanced transaction")]
#[simplex::fuzz]
fn unbalanced_transaction_is_a_setup_error(test_context: TestContext<FuzzMode>) {
    let mut scenario = vault_scenario(1);
    scenario.add::<VaultProgram>("vault").spends(0);

    test_context.engine(scenario).mutations(Mutations::default()).run();
}

/// Input 0 enables the locktime, which the template sets to 100.
#[simplex::fuzz]
fn timelock_passes_with_a_spec_on_the_lock_height(test_context: TestContext<FuzzMode>) {
    let mut transaction = FinalTransaction::new();
    transaction.add_input(
        PartialInput::default().with_sequence(Sequence::ENABLE_LOCKTIME_NO_RBF),
        None,
    );
    transaction.set_locktime(LockTime::from_consensus(100));

    let mut scenario = Scenario::new(transaction);
    scenario
        .add::<TimelockProgram>("timelock")
        .spends(0)
        .bind_arguments(|arguments, _| arguments.lock_height = 100);

    test_context
        .engine(scenario)
        .mutations(Mutations::none().timelocks())
        .spec(|case| {
            if case.lock_height() >= 100 {
                Expect::Accept
            } else {
                Expect::Reject
            }
        })
        .run();
}

/// Input 0 is the program's and output 0 pays the script it expects.
#[simplex::fuzz]
fn paired_output_is_judged_under_layout(test_context: TestContext<FuzzMode>) {
    let mut transaction = FinalTransaction::new();
    transaction.add_input(PartialInput::default(), None);
    transaction.add_output(PartialOutput::new(
        Script::new_op_return(b"next"),
        0,
        AssetId::default(),
    ));

    let mut scenario = Scenario::new(transaction);
    scenario
        .add::<PairedOutputProgram>("paired_output")
        .spends(0)
        .bind_arguments(|arguments, case| arguments.next_script_hash = case.output_script_hash(case.output_index(0)));

    test_context
        .engine(scenario)
        .mutations(Mutations::none().layout().everywhere())
        .spec(|case| {
            if case.input_index(0) == case.output_index(0) {
                Expect::Accept
            } else {
                Expect::Reject
            }
        })
        .run();
}

/// Input 1 holds the auth UTXO the witness points at.
#[simplex::fuzz]
fn auth_input_requirement_follows_its_utxo(test_context: TestContext<FuzzMode>) {
    let auth = UTXO::explicit(
        OutPoint::default(),
        Script::new_op_return(b"auth"),
        0,
        AssetId::default(),
    );

    let mut transaction = FinalTransaction::new();
    transaction.add_input(PartialInput::default(), None);
    transaction.add_input(PartialInput::new(auth), None);

    let mut scenario = Scenario::new(transaction);
    scenario
        .add::<ScriptAuthProgram>("script_auth")
        .spends(0)
        .bind_arguments(|arguments, case| arguments.auth_script_hash = case.input_script_hash(case.input_index(1)))
        .requires_witness(|witness, case| witness.auth_input = case.input_index(1) as u32);

    test_context
        .engine(scenario)
        .mutations(Mutations::none().layout())
        .run();
}

/// The transaction enables the locktime, so a final input has to override it.
#[simplex::fuzz]
fn sequence_mutation_reaches_the_signed_transaction(test_context: TestContext<FuzzMode>) {
    let mut transaction = FinalTransaction::new();
    transaction.set_sequence(Sequence::ENABLE_LOCKTIME_NO_RBF);
    transaction.add_input(PartialInput::default(), None);

    let mut scenario = Scenario::new(transaction);
    scenario.add::<DummyAcceptProgram>("accepting").spends(0);

    test_context
        .engine(scenario)
        .mutations(Mutations::none().timelocks())
        .spec(|case| {
            let mutated = case.mutations().iter().rev().find_map(|mutation| match mutation {
                Mutation::SetSequence { input: 0, sequence } => Some(*sequence),
                _ => None,
            });

            if let Some(sequence) = mutated {
                let signed = case.transaction().extract_pst().unwrap().0.extract_tx().unwrap();

                assert_eq!(signed.input[0].sequence, sequence);
                assert_eq!(case.input_sequence(0), sequence);
            }

            Expect::Accept
        })
        .run();
}
