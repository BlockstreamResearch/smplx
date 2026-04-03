use simplex::transaction::{FinalTransaction, PartialOutput};

#[simplex::test]
fn confidential_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let alice = context.get_default_signer();
    let bob = context.create_signer("sing slogan bar group gauge sphere rescue fossil loyal vital model desert");

    let mut ft = FinalTransaction::new();

    ft.add_output(
        PartialOutput::new(
            bob.get_address().script_pubkey(),
            100,
            context.get_network().policy_asset(),
        )
        .with_blinding_key(bob.get_blinding_public_key()),
    );

    let tx = alice.broadcast(&ft)?;
    println!("Broadcast: {}", tx);

    provider.wait(&tx)?;
    println!("Confirmed");

    let tx = bob.send(alice.get_address().script_pubkey(), 50)?;
    println!("Broadcast: {}", tx);

    provider.wait(&tx)?;
    println!("Confirmed");

    Ok(())
}
