use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, FuzzCase, Mutations, Scenario};
use simplex::simplicityhl::elements::{AssetId, LockTime, OutPoint, Script, Sequence};
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::conserving_vault::ConservingVaultProgram;
use simplex_fixtures::artifacts::timelock::TimelockProgram;
use simplex_fixtures::artifacts::unvault::UnvaultProgram;
use simplex_fixtures::artifacts::vault::VaultProgram;

/// Input 0 holds 1000 of the vault's coins, output 0 pays them to the unvault and output 1 is the fee.
fn vault_scenario(fee: u64) -> Scenario {
    let asset = AssetId::from_byte_array([1; 32]);

    let mut transaction = FinalTransaction::new();
    transaction.add_input(
        PartialInput::new(UTXO::explicit(OutPoint::default(), Script::new(), 1000, asset)),
        RequiredSignature::None,
    );
    transaction.add_output(PartialOutput::new(Script::new(), 1000 - fee, asset));
    transaction.add_output(PartialOutput::new(Script::new(), 0, asset));

    let mut scenario = Scenario::new(transaction);
    scenario.add::<UnvaultProgram>("unvault").locks(0).random_strategy();

    scenario
}

/// Output 0 pays the whole input, in its asset, to the unvault.
fn pays_unvault(case: &FuzzCase<'_>) -> Expect {
    let unvault = case.program::<UnvaultProgram>("unvault");

    if case.output_script_hash(0) == unvault.script_hash()
        && case.output_amount(0) == case.input_amount(0)
        && case.output_asset(0) == case.input_asset(0)
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
        .mutations(Mutations::default())
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
        RequiredSignature::None,
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
