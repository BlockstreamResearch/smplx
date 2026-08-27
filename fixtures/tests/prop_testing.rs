mod failure_test_prop {
    use simplex::fuzz;
    use simplex::fuzz::builders::{FinalTransactionBuilder, ProgramTarget};
    use simplex::fuzz::core::FuzzContext;
    use simplex::fuzz::engine::FuzzStrategyBuilder;
    use simplex::fuzz::{FuzzEngineBuilder, FuzzError, ProgramCheck, ProgramExecResult};
    use simplex::program::ProgramError;
    use simplex::provider::SimplicityNetwork;
    use simplex::simplicityhl::elements::hashes::Hash;
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::elements::{OutPoint, Script, TxOut, Txid};
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};

    use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
    use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};

    struct FailureTestCheck;

    const FAILURE_PROGRAM_TARGET: ProgramTarget = ProgramTarget::Input(0);

    fn failure_transaction_builder() -> Result<FinalTransactionBuilder, FuzzError> {
        let network = SimplicityNetwork::default_regtest();
        let utxo = UTXO {
            outpoint: OutPoint::new(Txid::all_zeros(), 0),
            txout: TxOut::new_fee(0, network.policy_asset()),
            secrets: None,
        };
        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::new(utxo), RequiredSignature::None);

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

    struct ScriptMismatchCheck;

    impl ProgramCheck for ScriptMismatchCheck {
        fn call(
            &self,
            _ctx: &FuzzContext,
            _tx: &PartiallySignedTransaction,
            _arguments: &Arguments,
            _witness: &WitnessValues,
            _input_index: usize,
            program_exec_result: ProgramExecResult,
        ) -> Result<(), String> {
            match program_exec_result {
                Err(ProgramError::ScriptPubkeyMismatch { .. }) => Ok(()),
                Err(error) => Err(format!("expected a script pubkey mismatch, got {error}")),
                Ok(_) => Err("expected execution to fail after replacing the program script".to_string()),
            }
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
        let transaction_builder = failure_transaction_builder()?;
        let runner = fuzz_engine.build(strategy_storage, transaction_builder);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[test]
    fn possible_interface_failure_program_with_pool() -> anyhow::Result<()> {
        let result = std::panic::catch_unwind(|| {
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
            let transaction_builder = failure_transaction_builder().unwrap();
            let runner = fuzz_engine_builder.build(strategy_storage, transaction_builder);
            runner.run_with_check(FailureTestCheck);
        });
        assert!(result.is_err());

        Ok(())
    }

    #[ignore]
    #[test]
    fn post_hook_can_replace_one_program_script() -> anyhow::Result<()> {
        let config = fuzz::proptest::test_runner::Config {
            test_name: Some(concat!(
                module_path!(),
                "::",
                stringify!(post_hook_can_replace_one_program_script)
            )),
            ..Default::default()
        };
        let fuzz_engine =
            FuzzEngineBuilder::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::new(config);
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random()
            .build();
        let transaction_builder =
            failure_transaction_builder()?.with_post_hook(|transaction, _program_script, _arguments, _witness| {
                FinalTransactionBuilder::set_program_script(transaction, FAILURE_PROGRAM_TARGET, &Script::new())
            });

        fuzz_engine
            .build(strategy_storage, transaction_builder)
            .run_with_check(ScriptMismatchCheck);

        Ok(())
    }

    #[simplex::fuzz]
    fn possible_interface_failure_program_with_interesting_values(
        fuzz_engine_builder: FuzzEngineBuilder<FailureTestProgram, FailureTestArguments, FailureTestWitness>,
    ) -> anyhow::Result<()> {
        // TODO: Add additional strategies to builder to make proper proptest
        let strategy_storage = FuzzStrategyBuilder::<FailureTestArguments, FailureTestWitness>::new().build();
        let transaction_builder = failure_transaction_builder()?;
        let runner = fuzz_engine_builder.build(strategy_storage, transaction_builder);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }
}
