use simplex::mutantesting;
use simplex::mutantesting::strategy::args::RandomValuePool;
use simplex::mutantesting::strategy::pset::{DefaultBaseContextGen, DefaultContextGen};
use simplex::mutantesting::{FuzzContext, ProgramCheck, ProgramExecResult, SimplexFuzzEngine};
use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
use simplex::simplicityhl::{Arguments, WitnessValues};
use simplex_fixtures::artifacts::failure_test::derived_failure_test::{FailureTestArguments, FailureTestWitness};
use simplex_fixtures::artifacts::failure_test::FailureTestProgram;
use std::marker::PhantomData;

mod simple_storage_test {
    use super::*;
    use std::fmt::Debug;

    use anyhow::Result;

    use crate::simple_storage_test::init::SimpleStorageInit;
    use simplex::mutantesting::core::{ArgGenFuzzStrategy, FuzzableBaseContextGen};
    use simplex::program::{
        ArgumentsTrait, ProgramTrait, RandomArguments, RandomWitness, SimplexProgram, WitnessTrait,
    };
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::elements::AssetId;

    mod init {
        use super::*;
        use simplex::mutantesting::core::{ArgGenFuzzStrategy2, FuzzableBaseContextGen};
        use simplex::mutantesting::strategy::args::PstBuilder;
        use simplex::mutantesting::prop::test_runner::TestRng;
        use simplex::mutantesting::{BoxedStrategy, FuzzableProgram, NewTree, RandomValueTree, Strategy, TestRunner};
        use simplex::program::{ArgumentsTrait, WitnessTrait};
        use simplex::provider::SimplicityNetwork;
        use simplex::rand::Rng;
        use simplex::simplicityhl::elements::hashes::Hash;
        use simplex::simplicityhl::elements::pset::serialize::Serialize;
        use simplex::simplicityhl::elements::{OutPoint, TxOut, Txid};
        use simplex::transaction::{
            FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature, UTXO,
        };
        use simplex_fixtures::artifacts::simple_storage::derived_simple_storage::{
            SimpleStorageArguments, SimpleStorageWitness,
        };
        use std::fmt::Formatter;

        #[derive(Clone)]
        pub struct SimpleStorageInit<Args, Wit> {
            phantom_data: PhantomData<(Args, Wit)>,
        }

        impl<Args, Wit> Default for SimpleStorageInit<Args, Wit> {
            fn default() -> Self {
                Self {
                    phantom_data: PhantomData,
                }
            }
        }

        impl<T, E> Debug for SimpleStorageInit<T, E> {
            fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
                writeln!(f, "SimpleStorageInit...")
            }
        }

        impl<Args: RandomArguments + std::fmt::Debug, Wit: RandomWitness + std::fmt::Debug> Strategy
            for SimpleStorageInit<Args, Wit>
        {
            type Tree = RandomValueTree<(Arguments, WitnessValues)>;
            type Value = (Arguments, WitnessValues);

            fn new_tree(&self, runner: &mut TestRunner) -> NewTree<Self> {
                Ok(RandomValueTree((
                    Args::generate_arguments(runner.rng()),
                    Wit::generate_witness(runner.rng()),
                )))
            }
        }

        impl<
            Args: RandomArguments + std::fmt::Debug + Clone + 'static,
            Wit: RandomWitness + std::fmt::Debug + Clone + 'static,
        > ArgGenFuzzStrategy<Args, Wit> for SimpleStorageInit<Args, Wit>
        {
            fn get_strategy(&self, _test_context: std::sync::Arc<FuzzContext>) -> BoxedStrategy<(Arguments, WitnessValues)> {
                simplex::mutantesting::strategy::args::Random::<Args, Wit>::default().boxed()
            }
        }

        impl<Args, Wit> PstBuilder<Arguments, WitnessValues> for SimpleStorageInit<Args, Wit> {
            fn build_pst(&self, context: &FuzzContext, args: &Arguments, wit: &WitnessValues) -> (PartiallySignedTransaction, Arguments, WitnessValues) {
                let base_gen = SimpleStorageInitBaseContext::default();
                let mut rng = simplex::rand::prelude::StdRng::seed_from_u64(0);
                let (_, pst, modified_args, modified_wit) = base_gen.build_base_transaction_3(context, args.clone(), wit.clone(), &mut rng);
                (pst, modified_args, modified_wit)
            }
        }

         impl<
            Args: RandomArguments + std::fmt::Debug + Clone + 'static,
            Wit: RandomWitness + std::fmt::Debug + Clone + 'static,
        > ArgGenFuzzStrategy2<Args, Wit> for SimpleStorageInit<Args, Wit>
        {
            fn get_strategy(&self, test_context: std::sync::Arc<FuzzContext>) -> BoxedStrategy<(Arguments, WitnessValues, PartiallySignedTransaction)> {
                use simplex::mutantesting::prop::strategy::Strategy;


                let init_strategy = (
                    simplex::mutantesting::strategy::args::Random::<Args, Wit>::default(),
                    0..(u32::MAX as u64),
                );

                let flat_strategy = init_strategy.prop_flat_map(move |((args, wit), old_value)| {
                    (
                        simplex::mutantesting::prop::strategy::Just(args),
                        simplex::mutantesting::prop::strategy::Just(wit),
                        simplex::mutantesting::prop::strategy::Just(old_value),
                        old_value..(u32::MAX as u64),
                    )
                });

                let result_strategy = flat_strategy.prop_map(move |(args, wit, old_value, new_value)| {
                    let mut ft = FinalTransaction::new();
                    let mut args_typed = SimpleStorageArguments::from_arguments(&args).unwrap();
                    let mut wit_typed = SimpleStorageWitness::from_witness(&wit).unwrap();

                    wit_typed.new_value = new_value as u64;

                    {
                        let mut slot: [u8; 32] = Default::default();
                        slot.copy_from_slice(&test_context.network.policy_asset().serialize());
                        args_typed.slot_id = slot;
                        args_typed.user = test_context.signer.as_ref().unwrap().get_schnorr_public_key().serialize();
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

                    let signer = test_context.signer.as_ref().unwrap();
                    let pst = signer.sign_tx_raw(&ft).unwrap();
                    let wit_signed = signer
                        .get_signed_program_witness(
                            &pst,
                            SimpleStorageProgram::new(modified_args.clone()).get_program(),
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

        pub struct SimpleStorageInitCheck;

        impl ProgramCheck for SimpleStorageInitCheck {
            fn call(
                &self,
                ctx: &FuzzContext,
                tx: &PartiallySignedTransaction,
                arguments: &Arguments,
                witness: &WitnessValues,
                program_exec_result: ProgramExecResult,
            ) -> std::result::Result<(), String> {
                if let Err(x) = program_exec_result {
                    Err(format!("some error: {x:?}"))
                } else {
                    Ok(())
                }
            }
        }

        #[derive(Default)]
        pub struct SimpleStorageInitBaseContext {}

        impl SimpleStorageInitBaseContext {
            pub fn build_base_transaction_3(
                &self,
                context: &FuzzContext,
                args: Arguments,
                wit: WitnessValues,
                rng: &mut StdRng,
            ) -> (FinalTransaction, PartiallySignedTransaction, Arguments, WitnessValues) {
                dbg!("Execution of build_base_transaction_3");
                let mut ft = FinalTransaction::new();
                let mut args_typed = SimpleStorageArguments::from_arguments(&args).unwrap();
                let mut wit_typed = SimpleStorageWitness::from_witness(&wit).unwrap();

                let max_amount = 2_100_000_000_000_000u64;
                let old_value = rng.random_range(0..max_amount / 2);
                let new_value = rng.random_range(old_value..max_amount);

                println!("new: {new_value} - old: {old_value}");

                wit_typed.new_value = new_value;

                {
                    let mut slot: [u8; 32] = Default::default();
                    slot.copy_from_slice(&context.network.policy_asset().serialize());
                    args_typed.slot_id = slot;
                    args_typed.user = context.signer.as_ref().unwrap().get_schnorr_public_key().serialize();
                }
                let modified_args = args_typed.build_arguments();
                let (_fuzz_program, old_storage_args_script) =
                    SimpleStorageProgram::build_program(modified_args.clone(), &context.network);

                ft.add_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
                        txout: {
                            let mut r = TxOut::new_fee(old_value, context.network.policy_asset());
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
                    asset: context.network.policy_asset(),
                    blinding_key: None,
                });
                ft.add_output(PartialOutput {
                    script_pubkey: old_storage_args_script,
                    amount: 0,
                    asset: Default::default(),
                    blinding_key: None,
                });

                let signer = context.signer.as_ref().unwrap();
                let pst = signer.sign_tx_raw(&ft).unwrap();
                let wit_signed = signer
                    .get_signed_program_witness(
                        &pst,
                        SimpleStorageProgram::new(modified_args.clone()).get_program(),
                        &wit_typed.build_witness(),
                        "USER_SIGNATURE",
                        &[],
                        0,
                    )
                    .unwrap();

                (ft, pst, modified_args, wit_signed)
            }


            // TODO: impossible to get a witness out of sign_tx_raw method
            // pub fn build_base_transaction_3(
            //     &self,
            //     context: &FuzzContext,
            //     args: Arguments,
            //     wit: WitnessValues,
            //     rng: &mut StdRng,
            // ) -> (FinalTransaction, PartiallySignedTransaction, Arguments, WitnessValues) {
            //     let mut ft = FinalTransaction::new();
            //     let mut args_typed = SimpleStorageArguments::from_arguments(&args).unwrap();
            //     let mut wit_typed = SimpleStorageWitness::from_witness(&wit).unwrap();
            //
            //     let max_amount = 2_100_000_000_000_000u64;
            //     let old_value = rng.random_range(0..max_amount / 2);
            //     let new_value = rng.random_range(old_value..max_amount);
            //
            //     println!("new: {new_value} - old: {old_value}");
            //
            //     wit_typed.new_value = new_value;
            //
            //     {
            //         let mut slot: [u8; 32] = Default::default();
            //         slot.copy_from_slice(&context.network.policy_asset().serialize());
            //         args_typed.slot_id = slot;
            //         args_typed.user = context.signer.as_ref().unwrap().get_schnorr_public_key().serialize();
            //     }
            //     let modified_args = args_typed.build_arguments();
            //     let (_fuzz_program, old_storage_args_script) =
            //         SimpleStorageProgram::build_program(modified_args.clone(), &context.network);
            //
            //     // ft.add_input(
            //     //     PartialInput::new(UTXO {
            //     //         outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
            //     //         txout: {
            //     //             let mut r = TxOut::new_fee(old_value, context.network.policy_asset());
            //     //             r.script_pubkey = old_storage_args_script.clone();
            //     //             r
            //     //         },
            //     //         secrets: None,
            //     //     }),
            //     //     RequiredSignature::None,
            //     // );
            //
            //     let box_program: Box<dyn ProgramTrait> =
            //         Box::new(SimpleStorageProgram::new(modified_args.clone()).get_program());
            //
            //     ft.add_program_input(
            //         PartialInput::new(UTXO {
            //             outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
            //             txout: {
            //                 let mut r = TxOut::new_fee(old_value, context.network.policy_asset());
            //                 r.script_pubkey = old_storage_args_script.clone();
            //                 r
            //             },
            //             secrets: None,
            //         }),
            //         ProgramInput::new(box_program, wit_typed.clone()),
            //         RequiredSignature::None,
            //     );
            //
            //     ft.add_input(
            //         PartialInput::new(UTXO {
            //             outpoint: OutPoint::new(Txid::from_slice(&[2; 32]).unwrap(), 1),
            //             txout: {
            //                 let mut r = TxOut::new_fee(1, AssetId::default());
            //                 r.script_pubkey = old_storage_args_script.clone();
            //                 r
            //             },
            //             secrets: None,
            //         }),
            //         RequiredSignature::None,
            //     );
            //
            //     ft.add_output(PartialOutput {
            //         script_pubkey: old_storage_args_script.clone(),
            //         amount: new_value,
            //         asset: context.network.policy_asset(),
            //         blinding_key: None,
            //     });
            //     ft.add_output(PartialOutput {
            //         script_pubkey: old_storage_args_script,
            //         amount: 0,
            //         asset: Default::default(),
            //         blinding_key: None,
            //     });
            //
            //     let signer = context.signer.as_ref().unwrap();
            //     let pst = signer.sign_tx_raw(&ft).unwrap();
            //     // let wit_signed = signer
            //     //     .get_signed_program_witness(
            //     //         &pst,
            //     //         SimpleStorageProgram::new(modified_args.clone()).get_program(),
            //     //         &wit_typed.build_witness(),
            //     //         "USER_SIGNATURE",
            //     //         &[],
            //     //         0,
            //     //     )
            //     //     .unwrap();
            //
            //     (ft, pst, modified_args, wit_signed)
            // }
        }

        impl<FuzzProgram: FuzzableProgram<FuzzProgram>> FuzzableBaseContextGen<FuzzProgram> for SimpleStorageInitBaseContext {
            // TODO: move base transaction creation into Tree strategy
            fn build_base_transaction(
                &self,
                network: &SimplicityNetwork,
                args: Arguments,
                wit: WitnessValues,
            ) -> FinalTransaction {
                todo!("FuzzableBaseContextGen::build_base_transaction")
            }

            fn build_base_transaction_2(
                &self,
                context: &FuzzContext,
                args: Arguments,
                wit: WitnessValues,
                rng: &mut TestRng,
            ) -> (FinalTransaction, PartiallySignedTransaction, Arguments, WitnessValues) {
                let mut ft = FinalTransaction::new();
                let mut args_typed = SimpleStorageArguments::from_arguments(&args).unwrap();
                let mut wit_typed = SimpleStorageWitness::from_witness(&wit).unwrap();

                let max_amount = 2_100_000_000_000_000u64;
                let old_value = rng.random_range(0..max_amount / 2);
                let new_value = rng.random_range(old_value..max_amount);

                println!("new: {new_value} - old: {old_value}");

                wit_typed.new_value = new_value;

                {
                    let mut slot: [u8; 32] = Default::default();
                    slot.copy_from_slice(&context.network.policy_asset().serialize());
                    args_typed.slot_id = slot;
                    args_typed.user = context.signer.as_ref().unwrap().get_schnorr_public_key().serialize();
                }
                let modified_args = args_typed.build_arguments();
                let (_fuzz_program, old_storage_args_script) =
                    SimpleStorageProgram::build_program(modified_args.clone(), &context.network);

                ft.add_input(
                    PartialInput::new(UTXO {
                        outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
                        txout: {
                            let mut r = TxOut::new_fee(old_value, context.network.policy_asset());
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
                    asset: context.network.policy_asset(),
                    blinding_key: None,
                });
                ft.add_output(PartialOutput {
                    script_pubkey: old_storage_args_script,
                    amount: 0,
                    asset: Default::default(),
                    blinding_key: None,
                });

                let signer = context.signer.as_ref().unwrap();
                let pst = signer.sign_tx_raw(&ft).unwrap();
                let wit_signed = signer
                    .get_signed_program_witness(
                        &pst,
                        SimpleStorageProgram::new(modified_args.clone()).get_program(),
                        &wit_typed.build_witness(),
                        "USER_SIGNATURE",
                        &[],
                        0,
                    )
                    .unwrap();

                (ft, pst, modified_args, wit_signed)
            }
        }
    }

    use init::*;
    use simplex::mutantesting::provider::MockProvider;
    use simplex::mutantesting::FuzzableProgram;
    use simplex::mutantesting::strategy::args::PstBuilder;
    use simplex::program::logger::ProgramLogger;
    use simplex::provider::SimplicityNetwork;
    use simplex::rand::prelude::StdRng;
    use simplex::rand::SeedableRng;
    use simplex::signer::Signer;
    use simplex_fixtures::artifacts::simple_storage::derived_simple_storage::{
        SimpleStorageArguments, SimpleStorageWitness,
    };
    use simplex_fixtures::artifacts::simple_storage::SimpleStorageProgram;

    #[test]
    fn test_simple_storage_mint_path() -> Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness>::from_config(
                mutantesting::Config::default(),
                PhantomData,
            );

        fuzz_engine.with_default_signer();
        fuzz_engine.with_pset_base_gen_strategy::<SimpleStorageInitBaseContext>();
        fuzz_engine.with_arg_gen_strategy::<SimpleStorageInit<SimpleStorageArguments, SimpleStorageWitness>>();
        fuzz_engine.run_with_check_2(SimpleStorageInitCheck);
        Ok(())
    }

    #[test]
    fn test_simple_storage_mint_path_with_pset() -> Result<()> {
        let fuzz_engine =
            SimplexFuzzEngine::<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness>::from_config(
                mutantesting::Config::default(),
                PhantomData,
            );

        fuzz_engine.with_default_signer();
        fuzz_engine.with_pset_base_gen_strategy::<SimpleStorageInitBaseContext>();
        fuzz_engine.with_arg_gen_strategy_plus_pst::<SimpleStorageInit<SimpleStorageArguments, SimpleStorageWitness>>();
        fuzz_engine.run_with_check_3(SimpleStorageInitCheck);
        Ok(())
    }

    // #[test]
    // fn minimal_debug() {
    //     let mut rng = StdRng::seed_from_u64(0);
    //
    //     const DEFAULT_TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";
    //
    //     let network = SimplicityNetwork::default_regtest();
    //     let signer = Signer::new(DEFAULT_TEST_MNEMONIC, Box::new(MockProvider { network }));
    //
    // let context = FuzzContext {
    //     signer: Some(std::sync::Arc::new(signer)),
    //     mock_provider: std::sync::Arc::new(MockProvider { network }),
    //     network,
    // };
    //     let base_gen = SimpleStorageInitBaseContext::default();
    //     let args = SimpleStorageArguments::default();
    //     let wit = SimpleStorageWitness::default();
    //     let (ft, pst, args, wit) = base_gen.build_base_transaction_3(
    //         &context,
    //         args.clone().build_arguments(),
    //         wit.clone().build_witness(),
    //         &mut rng,
    //     );
    //     println!("args: {args}, wit: {wit}");
    //     let (failure_program, _script) = SimpleStorageProgram::build_program(args.clone(), &context.network);
    //
    //     ProgramLogger::flush_logs();
    //     let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);
    //     assert!(dbg!(exec_result).is_ok());
    // }

    #[test]
    fn minimal_debug_2() {
        use std::sync::Arc;

        const DEFAULT_TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

        let network = SimplicityNetwork::default_regtest();
        let signer = Signer::new(DEFAULT_TEST_MNEMONIC, Box::new(MockProvider { network }));

        let context = FuzzContext {
            signer: Some(Arc::new(signer)),
            mock_provider: Arc::new(MockProvider { network }),
            network,
        };
        let builder = SimpleStorageInit::<SimpleStorageArguments, SimpleStorageWitness>::default();
        let args = SimpleStorageArguments::default().build_arguments();
        let wit = SimpleStorageWitness::default().build_witness();
        let (pst, args, wit) = builder.build_pst(&context, &args, &wit);

        println!("args: {args}, wit: {wit}");
        let (failure_program, _script) = SimpleStorageProgram::build_program(args.clone(), &context.network);

        // ProgramLogger::flush_logs();
        let exec_result: ProgramExecResult = failure_program.get_program().execute(&pst, &wit, 0, &context.network);
        assert!(dbg!(exec_result).is_ok());
    }


    //get_signed_program_witness
    //utils - remove
    // mock provider - remove, use esplora
    // provider rs remove
    //         fuzz_engine.with_pset_strategy::<DefaultContextGen>(); - remove
    // doundry testing fuzz
    // fuzzcontext - remove, move testing config
    // network - itmachine network
    // simplex program, fuzzablel program
    // simplex program -> asref<program>
    // fuzzableprogram name simplify (FP)
    // return ordinary program
    // only 1 strategy
}

mod failure_test {
    use super::*;
    use simplex::mutantesting::strategy::args::Random;

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
