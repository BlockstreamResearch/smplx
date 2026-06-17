mod failure_test_prop {
    use simplex::mutantesting;
    use simplex::mutantesting::core::FuzzStrategy;
    use simplex::mutantesting::proptest::prelude::Strategy;
    use simplex::mutantesting::{
        FuzzContext, FuzzableProgram, ProgramCheck, ProgramExecResult, SimplexFuzzEngine, sign_or_extract,
    };
    use simplex::simplicityhl::elements::hashes::Hash;
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::elements::{OutPoint, TxOut, Txid};
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};
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
            let args = FailureTestArguments::from_arguments(_arguments)?;
            let witness = FailureTestWitness::from_witness(_witness)?;
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

    #[derive(Debug, Default)]
    struct FailureGenStrategy;
    impl FuzzStrategy<FailureTestProgram, FailureTestArguments, FailureTestWitness> for FailureGenStrategy {
        fn get_strategy(
            &self,
            test_context: FuzzContext,
        ) -> simplex::mutantesting::proptest::strategy::BoxedStrategy<(
            Arguments,
            WitnessValues,
            PartiallySignedTransaction,
        )> {
            let init_strategy = (simplex::mutantesting::strategy::args::Random::<
                FailureTestArguments,
                FailureTestWitness,
            >::default(),);

            let flat_strategy = init_strategy.prop_flat_map(move |args| {
                (
                    simplex::mutantesting::proptest::strategy::Just(args.0.0),
                    simplex::mutantesting::proptest::strategy::Just(args.0.1),
                )
            });

            let result_strategy = flat_strategy.prop_map(move |(args, wit)| {
                const DEFAULT_FAUCET: u64 = 1 << 32;

                let mut ft = FinalTransaction::new();
                let (mutated_args, mutated_wit) = (args.clone(), wit.clone());

                let (failure_program, failure_script) = FailureTestProgram::build_program(args, &test_context.network);

                let txout = {
                    let mut r = TxOut::new_fee(DEFAULT_FAUCET, test_context.network.policy_asset());
                    r.script_pubkey = failure_script;
                    r
                };

                ft.add_program_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::all_zeros(), 0),
                        txout,
                        secrets: None,
                    }),
                    ProgramInput::new(Box::new(failure_program.as_ref().as_ref().clone()), wit),
                    RequiredSignature::None,
                );

                let signer = test_context.signer.as_ref();
                let pst = sign_or_extract(signer, &ft).unwrap();

                (mutated_args, mutated_wit, pst)
            });

            result_strategy.boxed()
        }
    }

    #[derive(Debug, Default)]
    struct FailureGenStrategyWithRandomPool;
    impl FuzzStrategy<FailureTestProgram, FailureTestArguments, FailureTestWitness> for FailureGenStrategyWithRandomPool {
        fn get_strategy(
            &self,
            test_context: FuzzContext,
        ) -> simplex::mutantesting::proptest::strategy::BoxedStrategy<(
            Arguments,
            WitnessValues,
            PartiallySignedTransaction,
        )> {
            let init_strategy = (simplex::mutantesting::strategy::args::RandomValuePool::<
                FailureTestArguments,
                FailureTestWitness,
            >::default(),);

            let flat_strategy = init_strategy.prop_flat_map(move |args| {
                (
                    simplex::mutantesting::proptest::strategy::Just(args.0.0),
                    simplex::mutantesting::proptest::strategy::Just(args.0.1),
                )
            });

            let result_strategy = flat_strategy.prop_map(move |(args, wit)| {
                const DEFAULT_FAUCET: u64 = 1 << 32;

                let mut ft = FinalTransaction::new();
                let (mutated_args, mutated_wit) = (args.clone(), wit.clone());

                let (failure_program, failure_script) = FailureTestProgram::build_program(args, &test_context.network);

                let txout = {
                    let mut r = TxOut::new_fee(DEFAULT_FAUCET, test_context.network.policy_asset());
                    r.script_pubkey = failure_script;
                    r
                };

                ft.add_program_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::all_zeros(), 0),
                        txout,
                        secrets: None,
                    }),
                    ProgramInput::new(Box::new(failure_program.as_ref().as_ref().clone()), wit),
                    RequiredSignature::None,
                );

                let signer = test_context.signer.as_ref();
                let pst = sign_or_extract(signer, &ft).unwrap();

                (mutated_args, mutated_wit, pst)
            });

            result_strategy.boxed()
        }
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program() -> anyhow::Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::from_config(
                mutantesting::proptest::test_runner::Config::default(),
                PhantomData,
            );

        fuzz_engine.with_default_signer();
        fuzz_engine.with_arg_gen_strategy::<FailureGenStrategy>();

        fuzz_engine.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program_with_pool() -> anyhow::Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::from_config(
                mutantesting::proptest::test_runner::Config::default(),
                PhantomData,
            );

        fuzz_engine.with_default_signer();
        fuzz_engine.with_arg_gen_strategy::<FailureGenStrategyWithRandomPool>();

        fuzz_engine.run_with_check(FailureTestCheck);

        Ok(())
    }
}

mod simple_storage_test_prop {
    use simplex::mutantesting;
    use simplex::mutantesting::core::FuzzStrategy;
    use simplex::mutantesting::proptest::prelude::Strategy;
    use simplex::mutantesting::{
        FuzzContext, FuzzableProgram, ProgramCheck, ProgramExecResult, SimplexFuzzEngine, sign_or_extract,
    };
    use simplex::program::{ArgumentsTrait, WitnessTrait};
    use simplex::simplicityhl::elements::AssetId;
    use simplex::simplicityhl::elements::hashes::Hash;
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::elements::pset::serialize::Serialize;
    use simplex::simplicityhl::elements::{OutPoint, TxOut, Txid};
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::PartialOutput;
    use simplex::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};
    use simplex_fixtures::artifacts::simple_storage::SimpleStorageProgram;
    use simplex_fixtures::artifacts::simple_storage::derived_simple_storage::{
        SimpleStorageArguments, SimpleStorageWitness,
    };
    use std::marker::PhantomData;

    pub struct SimpleStorageCheck;

    impl ProgramCheck for SimpleStorageCheck {
        fn call(
            &self,
            _ctx: &FuzzContext,
            _tx: &PartiallySignedTransaction,
            _arguments: &Arguments,
            _witness: &WitnessValues,
            program_exec_result: ProgramExecResult,
        ) -> Result<(), String> {
            if let Err(x) = program_exec_result {
                Err(format!("some error: {x:?}"))
            } else {
                Ok(())
            }
        }
    }

    #[derive(Debug, Default)]
    struct SimpleStorageStrategy;

    impl FuzzStrategy<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness> for SimpleStorageStrategy {
        fn get_strategy(
            &self,
            test_context: FuzzContext,
        ) -> simplex::mutantesting::proptest::strategy::BoxedStrategy<(
            Arguments,
            WitnessValues,
            PartiallySignedTransaction,
        )> {
            let init_strategy = (
                simplex::mutantesting::strategy::args::Random::<SimpleStorageArguments, SimpleStorageWitness>::default(
                ),
                0..(u32::MAX as u64),
            );

            let flat_strategy = init_strategy.prop_flat_map(move |((args, wit), old_value)| {
                (
                    simplex::mutantesting::proptest::strategy::Just(args),
                    simplex::mutantesting::proptest::strategy::Just(wit),
                    simplex::mutantesting::proptest::strategy::Just(old_value),
                    old_value..(u32::MAX as u64),
                )
            });

            let result_strategy = flat_strategy.prop_map(move |(args, wit, old_value, new_value)| {
                let mut ft = FinalTransaction::new();
                let mut args_typed = SimpleStorageArguments::from_arguments(&args).unwrap();
                let mut wit_typed = SimpleStorageWitness::from_witness(&wit).unwrap();
                let signer = test_context.signer.as_ref();

                wit_typed.new_value = new_value;

                {
                    let mut slot: [u8; 32] = Default::default();
                    slot.copy_from_slice(&test_context.network.policy_asset().serialize());
                    args_typed.slot_id = slot;
                    args_typed.user = signer.as_ref().unwrap().get_schnorr_public_key().serialize();
                }
                let modified_args = args_typed.build_arguments();
                let (_fuzz_program, old_storage_args_script) =
                    SimpleStorageProgram::build_program(modified_args.clone(), &test_context.network);

                ft.add_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
                        txout: {
                            let mut r = TxOut::new_fee(old_value, test_context.network.policy_asset());
                            r.script_pubkey = old_storage_args_script.clone();
                            r
                        },
                        secrets: None,
                    }),
                    RequiredSignature::None,
                );

                ft.add_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::from_slice(&[2; 32]).unwrap(), 1),
                        txout: {
                            let mut r = TxOut::new_fee(1, AssetId::default());
                            r.script_pubkey = old_storage_args_script.clone();
                            r
                        },
                        secrets: None,
                    }),
                    RequiredSignature::None,
                );

                ft.add_output(PartialOutput {
                    script_pubkey: old_storage_args_script.clone(),
                    amount: new_value,
                    asset: test_context.network.policy_asset(),
                    blinding_key: None,
                });
                ft.add_output(PartialOutput {
                    script_pubkey: old_storage_args_script,
                    amount: 0,
                    asset: Default::default(),
                    blinding_key: None,
                });

                let pst = sign_or_extract(signer, &ft).unwrap();
                let wit_signed = signer
                    .as_ref()
                    .unwrap()
                    .get_signed_program_witness(
                        &pst,
                        SimpleStorageProgram::new(modified_args.clone()).as_ref(),
                        &wit_typed.build_witness(),
                        "USER_SIGNATURE",
                        &[],
                        0,
                    )
                    .unwrap();

                (modified_args, wit_signed, pst)
            });

            result_strategy.boxed()
        }
    }

    #[ignore]
    #[test]
    fn possible_interface_simple_program() -> anyhow::Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness>::from_config(
                mutantesting::proptest::test_runner::Config::default(),
                PhantomData,
            );

        fuzz_engine.with_default_signer();
        fuzz_engine.with_arg_gen_strategy::<SimpleStorageStrategy>();

        fuzz_engine.run_with_check(SimpleStorageCheck);

        Ok(())
    }
}
