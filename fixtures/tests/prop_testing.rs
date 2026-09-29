mod failure_test_prop {
    use simplex::fuzz;
    use simplex::fuzz::builders::{FinalTransactionBuilder, ProgramTarget};
    use simplex::fuzz::core::FuzzContext;
    use simplex::fuzz::engine::FuzzStrategyBuilder;
    use simplex::fuzz::{FuzzEngineBuilder, FuzzError, ProgramCheck, ProgramExecResult};
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};

    use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
    use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub enum Expect {
        Ok,
        Failure,
    }

    struct FailureTestCheck {
        expect: Expect,
    }

    const FAILURE_PROGRAM_TARGET: ProgramTarget = ProgramTarget::Input(0);

    fn failure_transaction_builder() -> Result<FinalTransactionBuilder, FuzzError> {
        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);

        FinalTransactionBuilder::new(transaction, [FAILURE_PROGRAM_TARGET])
    }

    impl ProgramCheck for FailureTestCheck {
        fn call(
            &self,
            _ctx: &FuzzContext,
            _tx: &PartiallySignedTransaction,
            _arguments: &Arguments,
            _witness: &WitnessValues,
            _input_index: usize,
            program_exec_result: ProgramExecResult,
        ) -> Result<(), String> {
            let args = FailureTestArguments::from_arguments(_arguments)?;
            let witness = FailureTestWitness::from_witness(_witness)?;
            if (args.failure_value == witness.cmp_value || program_exec_result.is_err()) && self.expect == Expect::Ok {
                return Err(format!(
                    "Failed contract, failure_value == cmp_value , {program_exec_result:?}"
                ));
            }
            Ok(())
        }
    }

    #[simplex::fuzz]
    fn test_failure_catching_after_fuzzing(
        fuzz_engine_builder: FuzzEngineBuilder<FailureTestProgram, FailureTestArguments, FailureTestWitness>,
    ) -> anyhow::Result<()> {
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
        let transaction_builder = failure_transaction_builder()?;
        let runner = fuzz_engine_builder.build(strategy_storage, transaction_builder);
        runner.run_with_check(FailureTestCheck {
            expect: Expect::Failure,
        });

        Ok(())
    }

    #[should_panic]
    #[simplex::fuzz]
    fn test_panic_after_fuzzing(
        fuzz_engine_builder: FuzzEngineBuilder<FailureTestProgram, FailureTestArguments, FailureTestWitness>,
    ) {
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
        let transaction_builder = failure_transaction_builder().unwrap();
        let runner = fuzz_engine_builder.build(strategy_storage, transaction_builder);
        runner.run_with_check(FailureTestCheck { expect: Expect::Ok });
    }
}
