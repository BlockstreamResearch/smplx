use simplex_explorer::ElementsRpcClient;
use simplex_test::SimplexUser;

#[simplex::simplex_macros::test]
// #[test]
fn test_execution() {
    assert!(true);
}

#[test]
fn test_invocation_tx_tracking() -> anyhow::Result<()> {
    use simplex_test::{ConfigOption, ElementsRpc};

    fn test_invocation_tx_tracking(rpc: ElementsRpc, user1: SimplexUser, user2: SimplexUser) -> anyhow::Result<()> {
        {
            todo!();
            Ok(())
        }
    }
    let rpc = ElementsRpc::init(ConfigOption::DefaultRegtest).unwrap();

    let user1 = ElementsRpcClient::create_wallet(rpc.as_ref(), None).unwrap();
    println!("{}", user1.name);

    // let user2 = ElementsRpcClient::create_wallet(rpc.as_ref());
    // ElementsRpcClient::fund(rpc.as_ref(), rpc.network().policy_asset(), )?;
    // user1.fund(DEFAULT_SAT_AMOUNT_FAUCET, );
    // user2_fund();
    // args.fund_users();

    // test_invocation_tx_tracking(rpc, user1, user2)
    Ok(())
}
