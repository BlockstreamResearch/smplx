use simplex::simplicityhl::Arguments;
use simplex::{FuzzMode, TestContext};

use simplex::fuzz::Scenario;
use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::proptest::prelude::Strategy;
use simplex::fuzz::proptest::test_runner::RngSeed;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

use simplex_example::artifacts::exceptional_contract::ExceptionalContractProgram;
use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[simplex::fuzz]
fn p2pk_fuzz_test(context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };
    let fixed_arguments: Arguments = (&arguments).into();
    let program = P2pkProgram::new(arguments).as_ref().clone();

    // Keep the signer's public key fixed while fuzzing the initial witness.
    let strategy = ArgsStrategyBuilder::<P2pkProgram>::new()
        .build()
        .prop_map(move |(_, witness)| (fixed_arguments.clone(), witness));

    // Ask the signer to replace each fuzzed `SIGNATURE` with a valid signature.
    let mut transaction = FinalTransaction::new();
    transaction.add_program_input(
        PartialInput::new(UTXO::default()),
        ProgramInput::new(Box::new(program), P2pkWitness::default()),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    let mut scenario = Scenario::new(transaction);
    scenario.add::<P2pkProgram>("p2pk").spends(0).strategy(strategy);

    context.engine(scenario).run();

    Ok(())
}

#[should_panic(expected = "const CMP_VALUE: u16 = 1337;")]
#[simplex::fuzz]
fn test_panic(test_context: TestContext<FuzzMode>) {
    let mut config = test_context.get_fuzz_config().clone();
    config.cases = 66_000;
    config.rng_seed = RngSeed::Fixed(0x0000_0034);

    let mut scenario = Scenario::default();
    scenario.add::<ExceptionalContractProgram>("exceptional").spends(0);

    test_context.engine(scenario).with_custom_config(config).run();
}
