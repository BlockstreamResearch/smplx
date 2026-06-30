mod failure_test_prop {
    use simplex::fuzz;
    use simplex::fuzz::builders::FinalTransactionBuilder;
    use simplex::fuzz::core::{FuzzContext, FuzzFinalTransactionBuilder};
    use simplex::fuzz::engine::FuzzStrategyBuilder;
    use simplex::fuzz::{FuzzEngineBuilder, ProgramCheck, ProgramExecResult};
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
    use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};

    struct FailureTestCheck;

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
            if args.failure_value == witness.cmp_value {
                return Err(format!(
                    "Failed contract, failure_value == cmp_value , {program_exec_result:?}"
                ));
            }
            if program_exec_result.is_err() {
                println!("error: {program_exec_result:?}");
                return Err(format!("Failed contract, error: {program_exec_result:?}"));
            }
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct FailureGenStrategy;

    impl FuzzFinalTransactionBuilder<FailureTestProgram, FailureTestArguments, FailureTestWitness> for FailureGenStrategy {
        fn get_initial_ft(&self) -> FinalTransactionBuilder {
            FinalTransactionBuilder::new().add_program_input(None)
        }
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program() -> anyhow::Result<()> {
        let config = fuzz::proptest::test_runner::Config {
            test_name: ::core::option::Option::Some(::core::concat!(
                ::core::module_path!(),
                "::",
                ::core::stringify!(possible_interface_failure_program)
            )),
            source_file: Some(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/",
                stringify!(possible_interface_failure_program),
                ".txt"
            )),
            ..Default::default()
        };

        let fuzz_engine =
            FuzzEngineBuilder::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::new(config);

        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random()
            .build();
        let runner = fuzz_engine.with_no_signer().build(strategy_storage, FailureGenStrategy);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program_with_pool() -> anyhow::Result<()> {
        let config = fuzz::proptest::test_runner::Config {
            test_name: ::core::option::Option::Some(::core::concat!(
                ::core::module_path!(),
                "::",
                ::core::stringify!(possible_interface_failure_program_with_pool)
            )),
            source_file: Some(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/src/",
                stringify!(possible_interface_failure_program_with_pool),
                ".txt"
            )),
            ..Default::default()
        };

        let fuzz_engine_builder =
            FuzzEngineBuilder::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::new(config);

        // TODO: Add additional strategies to builder to make proper proptest
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random_pool()
            .build();
        let runner = fuzz_engine_builder
            .with_no_signer()
            .build(strategy_storage, FailureGenStrategy);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[simplex::fuzz]
    fn possible_interface_failure_program_with_interesting_values(
        fuzz_engine_builder: FuzzEngineBuilder<FailureTestProgram, FailureTestArguments, FailureTestWitness>,
    ) -> anyhow::Result<()> {
        // TODO: Add additional strategies to builder to make proper proptest
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random_interesting_values()
            .build();
        let runner = fuzz_engine_builder
            .with_no_signer()
            .build(strategy_storage, FailureGenStrategy);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }
}
