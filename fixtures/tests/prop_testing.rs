use simplex::mutantesting;
use simplex::mutantesting::strategy::args::{Random, RandomValuePool};
use simplex::mutantesting::strategy::pset::{DefaultBaseContextGen, DefaultContextGen};
use simplex::mutantesting::{FuzzContext, ProgramCheck, ProgramExecResult, SimplexFuzzEngine};
use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
use simplex::simplicityhl::{Arguments, WitnessValues};
use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};
use std::marker::PhantomData;

struct FailureTestCheck;

impl ProgramCheck for FailureTestCheck {
    fn call(
        &self,
        _ctx: &FuzzContext,
        _tx: &PartiallySignedTransaction,
        _arguments: &Arguments,
        _witness: &WitnessValues,
        program_exec_result: ProgramExecResult,
    ) -> Result<(), String> {
        // dbg!(&program_exec_result);
        let args = FailureTestArguments::from_arguments(_arguments).unwrap();
        let witness = FailureTestWitness::from_witness(_witness).unwrap();
        if args.failure_value == witness.cmp_value {
            return Err(format!("Failed contract, {program_exec_result:?}"));
        }
        if program_exec_result.is_err() {
            println!("error: {program_exec_result:?}");
            return Err(format!("Failed contract, {program_exec_result:?}"));
        }
        Ok(())
    }
}

mod tests {
    use super::*;
    #[ignore]
    #[test]
    fn possible_interface() -> anyhow::Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::from_config(
                mutantesting::Config::default(),
                PhantomData,
            );

        // TODO: REMOVE signer, seems to be redundant due to sign_tx method signature (it elides witness_utxo)
        fuzz_engine.with_default_signer();
        // TODO: move 2 lines into 1 strategy
        fuzz_engine.with_pset_base_gen_strategy::<DefaultBaseContextGen>();
        fuzz_engine.with_pset_strategy::<DefaultContextGen>();

        fuzz_engine.with_arg_gen_strategy::<Random<FailureTestArguments, FailureTestWitness>>();

        fuzz_engine.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[ignore]
    #[test]
    fn possible_interface_2() -> anyhow::Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::from_config(
                mutantesting::Config::default(),
                PhantomData,
            );

        // TODO: REMOVE signer, seems to be redundant due to sign_tx method signature (it elides witness_utxo)
        fuzz_engine.with_default_signer();
        // TODO: move 2 lines into 1 strategy
        fuzz_engine.with_pset_base_gen_strategy::<DefaultBaseContextGen>();
        fuzz_engine.with_pset_strategy::<DefaultContextGen>();

        fuzz_engine.with_arg_gen_strategy::<RandomValuePool<FailureTestArguments, FailureTestWitness>>();

        fuzz_engine.run_with_check(FailureTestCheck);

        Ok(())
    }
}
