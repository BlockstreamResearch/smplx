use simplex::simplicityhl::Arguments;
use simplex::{FuzzMode, TestContext};

use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::fuzz_program::Expect;
use simplex::fuzz::fuzz_transaction::FuzzTransaction;
use simplex::fuzz::proptest::prelude::Strategy;
use simplex::provider::SimplicityNetwork;
use simplex::signer::Signer;
use simplex::transaction::RequiredSignature;

use simplex_example::artifacts::exceptional_contract::ExceptionalContractProgram;
use simplex_example::artifacts::exceptional_contract::derived_exceptional_contract::{
    ExceptionalContractArguments, ExceptionalContractWitness,
};
use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[simplex::fuzz]
fn p2pk_fuzz_test(mut context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    const TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

    let signer = Signer::from_mnemonic(TEST_MNEMONIC, SimplicityNetwork::default_regtest());

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };
    let fixed_arguments: Arguments = (&arguments).into();

    context.set_custom_signer(signer);

    // Keep the signer's public key fixed while fuzzing the initial witness.
    let strategy = ArgsStrategyBuilder::<P2pkArguments, P2pkWitness>::new()
        .build()
        .prop_map(move |(_, witness)| (fixed_arguments.clone(), witness));

    let initial_transaction = FuzzTransaction::try_default()?.with_post_hook(|transaction, _, _, _| {
        // Ask the signer to replace each fuzzed `SIGNATURE` with a valid signature.
        transaction.inputs_mut()[0].required_sig = RequiredSignature::Witness("SIGNATURE".to_string());
        Ok(())
    });

    let runner = context.build::<P2pkProgram, P2pkArguments, P2pkWitness>(strategy, initial_transaction);

    runner.run(Expect::Ok);

    Ok(())
}

#[should_panic(expected = "const CMP_VALUE: u16 = 1337;")]
#[simplex::fuzz]
fn test_panic(mut test_context: TestContext<FuzzMode>) {
    let strategy = ArgsStrategyBuilder::<ExceptionalContractArguments, ExceptionalContractWitness>::new()
        .with_random()
        .build();
    let initial_transaction = FuzzTransaction::try_default().unwrap();

    test_context.set_cases(66_000);
    test_context.set_seed(0x0001);

    let runner = test_context
        .build::<ExceptionalContractProgram, ExceptionalContractArguments, ExceptionalContractWitness>(
            strategy,
            initial_transaction,
        );

    runner.run(Expect::Ok);
}
