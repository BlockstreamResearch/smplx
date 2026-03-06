#[simplex::test]
fn test_invocation_tx_tracking(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_provider();

    let fee = provider.get_fee_rate(1).unwrap();

    println!("{}", fee);

    Ok(())
}
