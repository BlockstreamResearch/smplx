#[simplex::test]
fn fetch_block_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let signer = context.get_default_signer();

    // The regtest provider mines a block for every broadcast, so the tip holds this transaction.
    let txid = signer.send(signer.get_address().script_pubkey(), 1_000)?.txid();

    let tip_hash = provider.fetch_tip_block_hash()?;
    let block = provider.fetch_block(&tip_hash)?;

    assert_eq!(block.block_hash().to_string(), tip_hash);
    assert_eq!(block.header.height, provider.fetch_tip_height()?);

    let txids: Vec<_> = block.txdata.iter().map(|tx| tx.txid()).collect();
    assert_eq!(txids, provider.fetch_block_txids(&tip_hash)?);

    let tx = block
        .txdata
        .iter()
        .find(|tx| tx.txid() == txid)
        .expect("the tip block holds the broadcast transaction");
    assert_eq!(*tx, provider.fetch_transaction(&txid)?);

    Ok(())
}
