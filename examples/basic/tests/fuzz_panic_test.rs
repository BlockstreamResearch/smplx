use simplex::fuzz::args_strategy::ArgsStrategyBuilder;
use simplex::fuzz::core::Expect;
use simplex::fuzz::transaction::FuzzTransaction;
use simplex::{FuzzMode, TestContext};

use simplex_example::artifacts::exceptional_contract::ExceptionalContractProgram;
use simplex_example::artifacts::exceptional_contract::derived_exceptional_contract::{
    ExceptionalContractArguments, ExceptionalContractWitness,
};

#[should_panic(expected = "const CMP_VALUE: u32 = 1;")]
#[simplex::fuzz]
fn test_panic(test_context: TestContext<FuzzMode>) {
    let strategy = ArgsStrategyBuilder::<ExceptionalContractArguments, ExceptionalContractWitness>::new().build();
    let initial_transaction = FuzzTransaction::try_default().unwrap();

    let runner = test_context
        .build::<ExceptionalContractProgram, ExceptionalContractArguments, ExceptionalContractWitness>(
            strategy,
            initial_transaction,
        );

    runner.run_with_default_check(Expect::Ok);
}
