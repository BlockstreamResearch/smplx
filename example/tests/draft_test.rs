use simplex::simplex_sdk::constants::SimplicityNetwork;
use simplex::simplex_test::{AddressType, ElementsRpcClient};
use simplex::simplex_test::{ConfigOption, TestClientProvider};
use simplex::simplex_test::{DEFAULT_SAT_AMOUNT_FAUCET, ElementsDConf};
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::secp256k1_zkp::Keypair;

#[ignore]
#[simplex::simplex_macros::test]
fn test_invocation_tx_tracking(test_context: simplex::simplex_test::TestContext) -> anyhow::Result<()> {
    let network = SimplicityNetwork::default_regtest();
    test_context.default_rpc_setup()?;

    let rpc_provider = test_context.get_rpc_provider();

    let user1_addr = rpc_provider.getnewaddress("", AddressType::default()).unwrap();
    let user2_addr = rpc_provider.getnewaddress("", AddressType::default()).unwrap();
    test_context.get_rpc_provider().sendtoaddress(
        &user1_addr,
        DEFAULT_SAT_AMOUNT_FAUCET,
        Some(network.policy_asset()),
    )?;

    test_context.get_rpc_provider().sendtoaddress(
        &user2_addr,
        DEFAULT_SAT_AMOUNT_FAUCET,
        Some(network.policy_asset()),
    )?;

    test_context.get_rpc_provider().generate_blocks(3)?;
    dbg!(test_context.get_rpc_provider().listunspent(
        None,
        None,
        Some(vec![user1_addr.to_string(), user2_addr.to_string()]),
        None,
        None,
    )?,);

    {
        let network = SimplicityNetwork::default_regtest();
        let keypair = Keypair::from_seckey_slice(&secp256k1::SECP256K1, &[1; 32])?;
        let p2pk = simplicityhl_core::get_p2pk_address(
            &keypair.x_only_public_key().0,
            simplicityhl_core::SimplicityNetwork::default_regtest(),
        )?;

        dbg!(p2pk.to_string());

        dbg!(test_context.get_rpc_provider().validateaddress(&p2pk.to_string())?);

        let result = test_context.get_rpc_provider().sendtoaddress(
            &p2pk,
            DEFAULT_SAT_AMOUNT_FAUCET,
            Some(network.policy_asset()),
        )?;

        test_context.get_rpc_provider().generate_blocks(5)?;

        dbg!(
            test_context
                .get_rpc_provider()
                .listunspent(None, None, Some(vec![p2pk.to_string()]), None, None,)?,
        );

        dbg!(
            test_context
                .get_rpc_provider()
                .scantxoutset("start", Some(vec![format!("addr({})", p2pk)]),)?,
        );

        Ok(())
    }
}
