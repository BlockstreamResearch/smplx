use std::time::SystemTime;

use simplex::NetworkUtils;

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

// Initializes the node to approximately time defined on next line
#[simplex::test(mock_time = 1296688602)]
fn test_mock_time(context: simplex::TestContext) -> anyhow::Result<()> {
    const SIX_MONTHS: u64 = 180 * 24 * 60 * 60;

    let network_utils = context.get_network_utils();
    let before = network_utils.get_blockchain_info()?;
    assert!(network_utils.set_mock_time(u64::try_from(before.median_time)?).is_err());
    assert_eq!(network_utils.get_blockchain_info()?.blocks, before.blocks);

    let target = before.median_time as u64 + SIX_MONTHS;

    network_utils.set_mock_time(target)?;

    let after = network_utils.get_blockchain_info()?;
    assert_eq!(after.blocks, before.blocks + NetworkUtils::BLOCKS_TO_ADVANCE_MTP as i64);
    assert_eq!(after.median_time, target as i64);
    assert_eq!(after.time, target as i64);

    // Set time now
    let system_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
    let threshold = 500;

    let before = network_utils.get_blockchain_info()?;
    network_utils.reset_mock_time()?;

    let after = network_utils.get_blockchain_info()?;
    assert_eq!(
        after.blocks,
        before.blocks + i64::try_from(NetworkUtils::BLOCKS_TO_ADVANCE_MTP)?
    );
    assert!(after.median_time < system_time as i64 + threshold && after.median_time > system_time as i64 - threshold);
    assert!(after.time < system_time as i64 + threshold && after.time > system_time as i64 - threshold);
    assert_eq!(after.time, after.median_time);

    Ok(())
}

// The time should be approximately equal to the machine's system time
#[simplex::test(mock_time = 0)]
fn test_mock_time_now_with_explicit_assign(context: simplex::TestContext) -> anyhow::Result<()> {
    let network_utils = context.get_network_utils();

    let system_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
    let threshold = 500;

    let now = network_utils.get_blockchain_info()?;
    assert!(now.median_time < system_time as i64 + threshold && now.median_time > system_time as i64 - threshold);
    assert!(now.time < system_time as i64 + threshold && now.time > system_time as i64 - threshold);
    assert_eq!(now.time, now.median_time);

    Ok(())
}

// The time should be approximately equal to the machine's system time
#[simplex::test]
fn test_mock_time_now_default(context: simplex::TestContext) -> anyhow::Result<()> {
    let network_utils = context.get_network_utils();

    let system_time = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)?.as_secs();
    let threshold = 500;

    let now = network_utils.get_blockchain_info()?;
    assert!(now.median_time < system_time as i64 + threshold && now.median_time > system_time as i64 - threshold);
    assert!(now.time < system_time as i64 + threshold && now.time > system_time as i64 - threshold);
    assert_eq!(now.time, now.median_time);

    Ok(())
}
