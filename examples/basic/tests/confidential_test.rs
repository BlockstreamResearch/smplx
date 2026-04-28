use simplex::simplicityhl::elements::AssetId;

use simplex::signer::Signer;
use simplex::transaction::partial_input::IssuanceInput;
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature, TxReceipt};

fn make_confidential_to_bob<'a>(alice: &'a Signer, bob: &Signer, asset: AssetId) -> anyhow::Result<TxReceipt<'a>> {
    let mut ft = FinalTransaction::new();

    ft.add_output(
        PartialOutput::new(bob.get_address().script_pubkey(), 1000, asset)
            .with_blinding_key(bob.get_blinding_public_key()),
    );

    let tx_receipt = alice.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

fn issue_confidential_to_alice<'a>(alice: &Signer, bob: &'a Signer) -> anyhow::Result<TxReceipt<'a>> {
    let utxos = bob.get_utxos()?;

    let mut ft = FinalTransaction::new();

    let issuance_details = ft.add_issuance_input(
        PartialInput::new(utxos[0].clone()),
        IssuanceInput::new_issuance(1000, 100, [1u8; 32]),
        RequiredSignature::NativeEcdsa,
    );

    ft.add_output(
        PartialOutput::new(alice.get_address().script_pubkey(), 1000, issuance_details.asset_id)
            .with_blinding_key(alice.get_blinding_public_key()),
    );
    ft.add_output(
        PartialOutput::new(
            alice.get_address().script_pubkey(),
            100,
            issuance_details.inflation_asset_id,
        )
        .with_blinding_key(alice.get_blinding_public_key()),
    );

    let tx_receipt = bob.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

#[simplex::test]
fn confidential_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let alice = context.get_default_signer();
    let bob = context.random_signer();

    let tx_receipt = make_confidential_to_bob(alice, &bob, provider.get_network().policy_asset())?;

    tx_receipt.wait()?;
    println!("Confirmed");

    let tx_receipt = issue_confidential_to_alice(alice, &bob)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    // spend confidential
    let tx_receipt = bob.send(alice.get_address().script_pubkey(), 50)?;
    println!("Broadcast: {}", tx_receipt);

    tx_receipt.wait()?;
    println!("Confirmed");

    Ok(())
}
