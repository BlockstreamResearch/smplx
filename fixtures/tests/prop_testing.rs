mod failure_test_prop {
    use simplex::fuzz::builders::{FuzzTransaction, ProgramTarget};
    use simplex::fuzz::core::FuzzContext;
    use simplex::fuzz::engine::FuzzStrategyBuilder;
    use simplex::fuzz::proptest::strategy::Just;
    use simplex::fuzz::{FuzzEngineBuilder, FuzzError, ProgramCheck, ProgramExecResult};
    use simplex::provider::SimplicityNetwork;
    use simplex::signer::Signer;
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

    use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
    use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};
    use simplex_fixtures::artifacts::p2pk::P2pkProgram;
    use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub enum Expect {
        Ok,
        Failure,
    }

    struct FailureTestCheck {
        expect: Expect,
    }

    struct SuccessfulProgramCheck;

    const FAILURE_PROGRAM_TARGET: ProgramTarget = ProgramTarget::Input(0);

    fn failure_transaction_builder() -> Result<FuzzTransaction, FuzzError> {
        let mut transaction = FinalTransaction::new();
        transaction.add_input(PartialInput::new(UTXO::default()), RequiredSignature::None);

        FuzzTransaction::new(transaction, [FAILURE_PROGRAM_TARGET])
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

    impl ProgramCheck for SuccessfulProgramCheck {
        fn call(
            &self,
            _ctx: &FuzzContext,
            _tx: &PartiallySignedTransaction,
            _arguments: &Arguments,
            witness: &WitnessValues,
            _input_index: usize,
            program_exec_result: ProgramExecResult,
        ) -> Result<(), String> {
            let signed_witness = P2pkWitness::from_witness(witness)?;
            if signed_witness == P2pkWitness::default() {
                return Err("fuzz check received the original unsigned witness".to_string());
            }

            program_exec_result
                .map(|_| ())
                .map_err(|error| format!("signed program failed: {error}"))
        }
    }

    fn signed_transaction_builder(
        arguments: &P2pkArguments,
        witness: &P2pkWitness,
    ) -> Result<FuzzTransaction, FuzzError> {
        let program = P2pkProgram::new(arguments.clone());
        let mut transaction = FinalTransaction::new();
        transaction.add_program_input(
            PartialInput::new(UTXO::default()),
            ProgramInput::new(Box::new(program.as_ref().clone()), witness),
            RequiredSignature::Witness("SIGNATURE".to_string()),
        );

        FuzzTransaction::new(transaction, [FAILURE_PROGRAM_TARGET])
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

    #[simplex::fuzz]
    fn test_signed_witness_with_program_checks(
        fuzz_engine_builder: FuzzEngineBuilder<P2pkProgram, P2pkArguments, P2pkWitness>,
    ) -> anyhow::Result<()> {
        const TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

        let signer = Signer::from_mnemonic(TEST_MNEMONIC, SimplicityNetwork::default_regtest());
        let arguments = P2pkArguments {
            public_key: signer.get_schnorr_public_key().serialize(),
        };
        let witness = P2pkWitness::default();
        let strategy_storage = Just(((&arguments).into(), (&witness).into()));
        let transaction_builder = signed_transaction_builder(&arguments, &witness)?;
        let runner = fuzz_engine_builder
            .with_signer(signer)
            .build(strategy_storage, transaction_builder);
        runner.run_with_check(SuccessfulProgramCheck);

        Ok(())
    }
}
