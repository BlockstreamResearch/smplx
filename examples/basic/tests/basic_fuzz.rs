use simplex::simplicityhl::Arguments;
use simplex::{FuzzMode, TestContext};

use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::fuzz_transaction::FuzzTransaction;
use simplex::fuzz::proptest::prelude::Strategy;
use simplex::fuzz::proptest::test_runner::RngSeed;
use simplex::transaction::RequiredSignature;

use simplex_example::artifacts::exceptional_contract::ExceptionalContractProgram;
use simplex_example::artifacts::exceptional_contract::derived_exceptional_contract::{
    ExceptionalContractArguments, ExceptionalContractWitness,
};
use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[simplex::fuzz]
fn p2pk_fuzz_test(context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };
    let fixed_arguments: Arguments = (&arguments).into();

    // Keep the signer's public key fixed while fuzzing the initial witness.
    let strategy = ArgsStrategyBuilder::<P2pkProgram>::new()
        .build()
        .prop_map(move |(_, witness)| (fixed_arguments.clone(), witness));

    let initial_transaction = FuzzTransaction::try_default()?.with_post_hook(|transaction, _, _, _| {
        // Ask the signer to replace each fuzzed `SIGNATURE` with a valid signature.
        transaction.inputs_mut()[0].required_sig = RequiredSignature::Witness("SIGNATURE".to_string());

        Ok(())
    });

    context
        .engine::<P2pkProgram, P2pkArguments, P2pkWitness>()
        .with_custom_strategy(strategy)
        .with_custom_transaction(initial_transaction)
        .run();

    Ok(())
}

#[should_panic(expected = "const CMP_VALUE: u16 = 1337;")]
#[simplex::fuzz]
fn test_panic(test_context: TestContext<FuzzMode>) {
    let mut config = test_context.get_fuzz_config().clone();
    config.cases = 66_000;
    config.rng_seed = RngSeed::Fixed(0x0000_0034);

    test_context
        .engine::<ExceptionalContractProgram, ExceptionalContractArguments, ExceptionalContractWitness>()
        .with_custom_config(config)
        .run();
}
