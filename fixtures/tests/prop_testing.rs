mod failure_test_prop {
    use simplex::mutantesting;
    use simplex::mutantesting::blueprint_constructor::BlueprintDraftConstructor;
    use simplex::mutantesting::core::ContractFuzzStrategyBlueprint;
    use simplex::mutantesting::engine::StrategyStorageBuilder;
    use simplex::mutantesting::{FuzzContext, FuzzStrategyBuilder, FuzzableProgram, ProgramCheck, ProgramExecResult};
    use simplex::simplicityhl::elements::hashes::Hash;
    use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
    use simplex::simplicityhl::elements::{OutPoint, TxOut, Txid};
    use simplex::simplicityhl::{Arguments, WitnessValues};
    use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};
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

    impl ContractFuzzStrategyBlueprint<FailureTestProgram, FailureTestArguments, FailureTestWitness>
        for FailureGenStrategy
    {
        fn get_initial_ft(&self) -> BlueprintDraftConstructor {
            BlueprintDraftConstructor::new().add_program_input(None)
        }
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program() -> anyhow::Result<()> {
        let config = mutantesting::proptest::test_runner::Config {
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
            FuzzStrategyBuilder::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::new(config);

        let strategy_storage = StrategyStorageBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random()
            .build();
        let runner = fuzz_engine.with_no_signer().build(strategy_storage, FailureGenStrategy);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }

    #[ignore]
    #[test]
    fn possible_interface_failure_program_with_pool() -> anyhow::Result<()> {
        let config = mutantesting::proptest::test_runner::Config {
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

        let fuzz_engine =
            FuzzStrategyBuilder::<FailureTestProgram, FailureTestArguments, FailureTestWitness>::new(config);

        let strategy_storage = StrategyStorageBuilder::<FailureTestArguments, FailureTestWitness, _>::new()
            .with_random_pool()
            .build();
        let runner = fuzz_engine.with_no_signer().build(strategy_storage, FailureGenStrategy);
        runner.run_with_check(FailureTestCheck);

        Ok(())
    }
}

// mod simple_storage_test_prop {
//     use simplex::mutantesting::blueprint_constructor::BlueprintDraftConstructor;
//     use simplex::mutantesting::core::{ContractFuzzStrategy, ContractFuzzStrategyBlueprint};
//     use simplex::mutantesting::engine::{get_default_provider, StrategyStorageBuilder};
//     use simplex::mutantesting::{FuzzContext, FuzzStrategyBuilder, FuzzableProgram, ProgramCheck, ProgramExecResult};
//     use simplex::program::{ArgumentsTrait, WitnessTrait};
//     use simplex::provider::SimplicityNetwork;
//     use simplex::signer::Signer;
//     use simplex::simplicityhl::elements::hashes::Hash;
//     use simplex::simplicityhl::elements::pset::serialize::Serialize;
//     use simplex::simplicityhl::elements::pset::PartiallySignedTransaction;
//     use simplex::simplicityhl::elements::AssetId;
//     use simplex::simplicityhl::elements::{OutPoint, TxOut, Txid};
//     use simplex::simplicityhl::{Arguments, WitnessValues};
//     use simplex::transaction::PartialOutput;
//     use simplex::transaction::{FinalTransaction, PartialInput, RequiredSignature, UTXO};
//     use simplex::{mutantesting, TestContext};
//     use simplex_fixtures::artifacts::simple_storage::derived_simple_storage::{
//         SimpleStorageArguments, SimpleStorageWitness,
//     };
//     use simplex_fixtures::artifacts::simple_storage::SimpleStorageProgram;
//     use std::path::PathBuf;
//
//     pub struct SimpleStorageCheck;
//
//     impl ProgramCheck for SimpleStorageCheck {
//         fn call(
//             &self,
//             _ctx: &FuzzContext,
//             _tx: &PartiallySignedTransaction,
//             _arguments: &Arguments,
//             _witness: &WitnessValues,
//             _input_index: usize,
//             program_exec_result: ProgramExecResult,
//         ) -> Result<(), String> {
//             if let Err(x) = program_exec_result {
//                 Err(format!("some error: {x:?}"))
//             } else {
//                 Ok(())
//             }
//         }
//     }
//
//     #[derive(Debug, Default)]
//     struct SimpleStorageStrategy;
//
//     impl ContractFuzzStrategyBlueprint<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness>
//         for SimpleStorageStrategy
//     {
//         type AdditionalInput = u64;
//
//         fn get_final_tx(
//             &self,
//             context: &FuzzContext,
//             args: &Arguments,
//             wit: &WitnessValues,
//             additional: &Self::AdditionalInput,
//         ) -> FinalTransaction {
//             let mut ft = FinalTransaction::new();
//             let mut args_typed = SimpleStorageArguments::from_arguments(&arguments).unwrap();
//             let mut wit_typed = SimpleStorageWitness::from_witness(&witness).unwrap();
//             let signer = test_context.get_signer();
//             let mut old_value = additional;
//             let new_value = if old_value == u64::MAX {
//                 let res = old_value;
//                 old_value -= 1;
//                 res
//             } else {
//                 old_value + 1
//             };
//
//             wit_typed.new_value = new_value;
//
//             {
//                 let mut slot: [u8; 32] = Default::default();
//                 slot.copy_from_slice(&test_context.network.policy_asset().serialize());
//                 args_typed.slot_id = slot;
//                 args_typed.user = signer.as_ref().unwrap().get_schnorr_public_key().serialize();
//             }
//             let modified_args = args_typed.build_arguments();
//             let (_fuzz_program, old_storage_args_script) =
//                 SimpleStorageProgram::build_program(modified_args.clone(), &test_context.network);
//
//             ft.add_input(
//                 PartialInput::new(UTXO {
//                     outpoint: OutPoint::new(Txid::from_slice(&[1; 32]).unwrap(), 0),
//                     txout: {
//                         let mut r = TxOut::new_fee(old_value, test_context.network.policy_asset());
//                         r.script_pubkey = old_storage_args_script.clone();
//                         r
//                     },
//                     secrets: None,
//                 }),
//                 RequiredSignature::None,
//             );
//
//             ft.add_input(
//                 PartialInput::new(UTXO {
//                     outpoint: OutPoint::new(Txid::from_slice(&[2; 32]).unwrap(), 1),
//                     txout: {
//                         let mut r = TxOut::new_fee(1, AssetId::default());
//                         r.script_pubkey = old_storage_args_script.clone();
//                         r
//                     },
//                     secrets: None,
//                 }),
//                 RequiredSignature::None,
//             );
//
//             ft.add_output(PartialOutput {
//                 script_pubkey: old_storage_args_script.clone(),
//                 amount: new_value,
//                 asset: test_context.network.policy_asset(),
//                 blinding_key: None,
//             });
//             ft.add_output(PartialOutput {
//                 script_pubkey: old_storage_args_script,
//                 amount: 0,
//                 asset: Default::default(),
//                 blinding_key: None,
//             });
//
//             // TODO: how to make correctly here?
//             let pst = test_context.sign_or_extract(&ft).unwrap();
//             let wit_signed = signer
//                 .as_ref()
//                 .unwrap()
//                 .get_signed_program_witness(
//                     &pst,
//                     SimpleStorageProgram::new(modified_args.clone()).as_ref(),
//                     &wit_typed.build_witness(),
//                     "USER_SIGNATURE",
//                     &[],
//                     0,
//                 )
//                 .unwrap();
//
//             ft
//         }
//
//         fn get_initial_ft(&self) -> BlueprintDraftConstructor {
//             BlueprintDraftConstructor::new()
//                 .add_program_custom_input()
//                 .add_program_custom_input()
//                 .add
//         }
//     }
//
//     // with_signer by default - false, inclusing value - true (bridging signer from config)
//     // #[simplex::proptest(program = "simplex_fixtures::artifacts::simple_storage::SimpleStorageProgram", with_signer)]
//
//     #[ignore]
//     #[test]
//     fn possible_interface_simple_program__smplx_test() -> anyhow::Result<()> {
//         fn possible_interface_simple_program__smplx_test(
//             builder: FuzzStrategyBuilder<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness, u64>,
//         ) -> anyhow::Result<()> {
//             let strategy_storage = StrategyStorageBuilder::<SimpleStorageArguments, SimpleStorageWitness, _, _>::new()
//                 .with_random()
//                 .with_random_asset_value()
//                 .build();
//             let runner = builder
//                 .with_signer({
//                     const DEFAULT_REGTEST_MNEMONIC: &str =
//                         "exist carry drive collect lend cereal occur much tiger just involve mean";
//                     Signer::new(
//                         DEFAULT_REGTEST_MNEMONIC,
//                         Box::new(get_default_provider(SimplicityNetwork::default_regtest())),
//                     )
//                 })
//                 .build(strategy_storage, SimpleStorageStrategy);
//             runner.run_with_check(SimpleStorageCheck);
//
//             Ok(())
//         }
//
//         const SIMPLEX_PROP_TEST_ENV: &str = "SIMPLEX_RUN_PROP_TESTS";
//
//         if std::env::var(SIMPLEX_PROP_TEST_ENV).is_ok() {
//             let test_context = match std::env::var("SIMPLEX_TEST_ENV") {
//                 Err(_) => {
//                     panic!("Failed to run this test, required to use `simplex test`");
//                 }
//                 Ok(path) => TestContext::new(PathBuf::from(path)).unwrap(),
//             };
//
//             let config = mutantesting::proptest::test_runner::Config {
//                 test_name: ::core::option::Option::Some(::core::concat!(
//                     ::core::module_path!(),
//                     "::",
//                     ::core::stringify!(possible_interface_simple_program)
//                 )),
//                 source_file: Some(concat!(
//                     env!("CARGO_MANIFEST_DIR"),
//                     "/src/",
//                     stringify!(possible_interface_simple_program),
//                     ".txt"
//                 )),
//                 ..Default::default()
//             };
//             let fuzz_context_builder = FuzzStrategyBuilder::from_context(config, test_context);
//
//             possible_interface_simple_program__smplx_test(fuzz_context_builder)
//         } else {
//             eprintln!(
//                 "Set '--proptest' flag in simplex to run a {} proptest",
//                 stringify!(possible_interface_simple_program)
//             );
//
//             let config = mutantesting::proptest::test_runner::Config {
//                 test_name: ::core::option::Option::Some(::core::concat!(
//                     ::core::module_path!(),
//                     "::",
//                     ::core::stringify!(possible_interface_simple_program)
//                 )),
//                 source_file: Some(concat!(
//                     env!("CARGO_MANIFEST_DIR"),
//                     "/src/",
//                     stringify!(possible_interface_simple_program),
//                     ".txt"
//                 )),
//                 ..Default::default()
//             };
//             let fuzz_context_builder = FuzzStrategyBuilder::new(config);
//
//             possible_interface_simple_program__smplx_test(fuzz_context_builder)
//
//             // todo!()
//         }
//     }
//
//     #[simplex::proptest]
//     fn possible_interface_simple_program_2(
//         fuzz_engine: FuzzStrategyBuilder<SimpleStorageProgram, SimpleStorageArguments, SimpleStorageWitness, u64>,
//     ) -> anyhow::Result<()> {
//         let strategy_storage = StrategyStorageBuilder::<SimpleStorageArguments, SimpleStorageWitness, _, _>::new()
//             .with_random_pool()
//             .with_random_asset_value()
//             .build();
//         // let blueprint = FuzzTxBlueprint::new().add_program_input().add_custom_step();
//         let runner = fuzz_engine
//             .with_signer({
//                 const DEFAULT_REGTEST_MNEMONIC: &str =
//                     "exist carry drive collect lend cereal occur much tiger just involve mean";
//                 Signer::new(
//                     DEFAULT_REGTEST_MNEMONIC,
//                     Box::new(get_default_provider(SimplicityNetwork::default_regtest())),
//                 )
//             })
//             .build(strategy_storage, SimpleStorageStrategy);
//         runner.run_with_check(SimpleStorageCheck);
//         Ok(())
//     }
//
//     // #[::core::prelude::v1::test]
//     // fn possible_interface_simple_program__smplx_test() -> anyhow::Result<()> {
//     //     use simplex::TestContext;
//     //     use std::path::PathBuf;
//     //     fn dummy_log_level__smplx_test(context: simplex::TestContext) -> anyhow::Result<()> {
//     //         {
//     //             let provider = context.get_default_provider();
//     //             let signer = context.get_default_signer();
//     //
//     //             let (dummy, script) = setup_dummy(&context);
//     //
//     //             let tx_receipt = signer.send(script.clone(), 50)?;
//     //             println!("Funded dummy script: {}", tx_receipt);
//     //
//     //             let utxos = provider.fetch_scripthash_utxos(&script)?;
//     //
//     //             let mut ft = FinalTransaction::new();
//     //
//     //             ft.add_program_input(
//     //                 PartialInput::new(utxos[0].clone()),
//     //                 ProgramInput::new(Box::new(dummy.as_ref().clone()), DummyPanicWitness::default()),
//     //                 RequiredSignature::None,
//     //             );
//     //
//     //             let result = signer.broadcast(&ft);
//     //
//     //             assert!(result.is_err(), "expected assert!(false) program to fail execution");
//     //             println!("{}", result.err().unwrap());
//     //
//     //             Ok(())
//     //         }
//     //     }
//     //     let test_context = match std::env::var("SIMPLEX_TEST_ENV") {
//     //         Err(_) => {
//     //             panic!("Failed to run this test, required to use `simplex test`");
//     //         }
//     //         Ok(path) => TestContext::new(PathBuf::from(path)).unwrap(),
//     //     };
//     //
//     //     dummy_log_level__smplx_test(test_context)
//     // }
// }
