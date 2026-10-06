use simplex::fuzz::core::Expect;
use simplex::fuzz::engine::FuzzStrategyBuilder;
use simplex::fuzz::transaction::FuzzTransaction;
use simplex::{FuzzMode, TestContext};

use simplex_example::artifacts::exceptional_contract::ExceptionalContractProgram;
use simplex_example::artifacts::exceptional_contract::derived_exceptional_contract::{
    ExceptionalContractArguments, ExceptionalContractWitness,
};

#[should_panic(expected = "const CMP_VALUE: u32 = 1;")]
#[simplex::fuzz]
fn test_panic(test_context: TestContext<FuzzMode>) {
    let strategy_storage =
        FuzzStrategyBuilder::<ExceptionalContractArguments, ExceptionalContractWitness>::new().build();
    let transaction_builder = FuzzTransaction::try_default().unwrap();

    let runner = test_context
        .build::<ExceptionalContractProgram, ExceptionalContractArguments, ExceptionalContractWitness>(
            strategy_storage,
            transaction_builder,
        );

    runner.run_with_default_check(Expect::Ok);
}
