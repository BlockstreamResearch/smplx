use simplex_provider::elements_rpc::{AddressType, ElementsRpcClient};
use simplex_sdk::constants::SimplicityNetwork;
use simplex_test::{DEFAULT_SAT_AMOUNT_FAUCET, TestContext};
use simplicityhl::elements::Address;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::secp256k1_zkp::Keypair;

#[ignore]
#[simplex::simplex_macros::test(hello = "hi")]
fn test_execution(x: TestContext) {
    assert!(true)
}

#[ignore]
#[test]
fn test_execution2() {
    use ::simplex::tracing;
    use simplex_test::TestContextBuilder;
    use std::path::PathBuf;

    fn test_execution2(x: TestContext) {
        assert!(true);
    }

    let test_context = match std::env::var("SIMPLEX_TEST_ENV") {
        Err(e) => {
            tracing::trace!(
                "Test 'test_in_custom_folder_custom_333' connected with simplex is disabled, run `simplex test` in order to test it, err: '{e}'"
            );
            panic!("Failed to run this test, required to use `simplex test`");
        }
        Ok(path) => {
            let path = PathBuf::from(path);
            let test_context = TestContextBuilder::FromConfigPath(path).build().unwrap();
            tracing::trace!("Running 'test_in_custom_folder_custom_333' with simplex configuration");
            test_context
        }
    };
    println!("fn name: {}, \n ident: {}", "test_execution2", "#ident");
    println!("input: {}, \n AttributeArgs: {}", "#input", "#args");

    test_execution2(test_context)
}

#[test]
fn test_invocation_tx_tracking() -> anyhow::Result<()> {
    use simplex_test::{ConfigOption, TestClientProvider};

    fn test_invocation_tx_tracking(
        rpc: TestClientProvider,
        user1_addr: Address,
        user2_addr: Address,
    ) -> anyhow::Result<()> {
        // user input code
        {
            let network = SimplicityNetwork::default_regtest();
            let keypair = Keypair::from_seckey_slice(&secp256k1::SECP256K1, &[1; 32])?;
            let p2pk = simplicityhl_core::get_p2pk_address(
                &keypair.x_only_public_key().0,
                simplicityhl_core::SimplicityNetwork::default_regtest(),
            )?;

            dbg!(p2pk.to_string());

            // simplex runtime
            // - test provider
            // - fields from config
            // -
            // p2tr

            // TODO: uncomment and fix
            dbg!(ElementsRpcClient::validateaddress(rpc.as_ref(), &p2pk.to_string())?);
            // ElementsRpcClient::importaddress(rpc.as_ref(), &p2pk.to_string(), None, None, None)?;

            // broadcast, fetch fee transaction

            let result = ElementsRpcClient::sendtoaddress(
                rpc.as_ref(),
                &p2pk,
                DEFAULT_SAT_AMOUNT_FAUCET,
                Some(network.policy_asset()),
            )?;

            ElementsRpcClient::generate_blocks(rpc.as_ref(), 5)?;

            dbg!(ElementsRpcClient::listunspent(
                rpc.as_ref(),
                None,
                None,
                Some(vec![p2pk.to_string()]),
                None,
                None,
            )?,);

            dbg!(ElementsRpcClient::scantxoutset(
                rpc.as_ref(),
                "start",
                Some(vec![format!("addr({})", p2pk)]),
            )?,);

            Ok(())
        }
    }

    let network = SimplicityNetwork::default_regtest();
    let rpc = TestClientProvider::init(ConfigOption::DefaultRegtest).unwrap();
    {
        ElementsRpcClient::generate_blocks(rpc.as_ref(), 1).unwrap();
        ElementsRpcClient::rescanblockchain(rpc.as_ref(), None, None).unwrap();
        ElementsRpcClient::sweep_initialfreecoins(rpc.as_ref()).unwrap();
        ElementsRpcClient::generate_blocks(rpc.as_ref(), 100).unwrap();
    }

    let user1_addr = ElementsRpcClient::getnewaddress(rpc.as_ref(), "", AddressType::default()).unwrap();
    let user2_addr = ElementsRpcClient::getnewaddress(rpc.as_ref(), "", AddressType::default()).unwrap();
    ElementsRpcClient::sendtoaddress(
        rpc.as_ref(),
        &user1_addr,
        DEFAULT_SAT_AMOUNT_FAUCET,
        Some(network.policy_asset()),
    )
    .unwrap();

    ElementsRpcClient::sendtoaddress(
        rpc.as_ref(),
        &user2_addr,
        DEFAULT_SAT_AMOUNT_FAUCET,
        Some(network.policy_asset()),
    )
    .unwrap();

    ElementsRpcClient::generate_blocks(rpc.as_ref(), 3).unwrap();
    dbg!(ElementsRpcClient::listunspent(
        rpc.as_ref(),
        None,
        None,
        Some(vec![user1_addr.to_string(), user2_addr.to_string()]),
        None,
        None,
    )?,);
    test_invocation_tx_tracking(rpc, user1_addr, user2_addr)
}
