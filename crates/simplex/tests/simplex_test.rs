use simplex_runtime::elements_rpc::{AddressType, ElementsRpcClient};
use simplex_test::DEFAULT_SAT_AMOUNT_FAUCET;
use simplicityhl::elements::Address;
use simplicityhl::elements::bitcoin::secp256k1;
use simplicityhl::elements::secp256k1_zkp::Keypair;

#[simplex::simplex_macros::test]
// #[test]
fn test_execution() {
    assert!(true);
}

#[test]
fn test_invocation_tx_tracking() -> anyhow::Result<()> {
    use simplex_test::{ConfigOption, TestProvider};

    fn test_invocation_tx_tracking(rpc: TestProvider, user1_addr: Address, user2_addr: Address) -> anyhow::Result<()> {
        // user input code
        {
            let network = rpc.network();
            let keypair = Keypair::from_seckey_slice(&secp256k1::SECP256K1, &[1; 32])?;
            let p2pk = simplex_core::get_p2pk_address(&keypair.x_only_public_key().0, network)?;

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
                Some(rpc.network().policy_asset()),
            );
            result?;

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
    let rpc = TestProvider::init(ConfigOption::DefaultRegtest).unwrap();
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
        Some(rpc.network().policy_asset()),
    )
    .unwrap();

    ElementsRpcClient::sendtoaddress(
        rpc.as_ref(),
        &user2_addr,
        DEFAULT_SAT_AMOUNT_FAUCET,
        Some(rpc.network().policy_asset()),
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
