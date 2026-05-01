#[simplex::test]
fn test_blocks_mining(context: simplex::TestContext) -> anyhow::Result<()> {
    const DESIRED_HEIGHT: u64 = 1_234;

    let network_utils = context.get_network_utils();
    network_utils.mine_until_height(DESIRED_HEIGHT)?;

    assert_eq!(
        DESIRED_HEIGHT,
        context.get_default_provider().fetch_tip_height()? as u64
    );

    Ok(())
}
