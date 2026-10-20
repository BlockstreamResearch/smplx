use simplex::{FuzzMode, TestContext};

use simplex::fuzz::{Expect, FuzzCase, Mutations, Scenario};
use simplex::simplicityhl::elements::{AssetId, OutPoint, Script};
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature, UTXO};

use simplex_fixtures::artifacts::conserving_vault::ConservingVaultProgram;
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
    let mut scenario = vault_scenario(0);

    scenario
        .add::<VaultProgram>("vault")
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
