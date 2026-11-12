use simplex::simplicityhl::elements::AssetId;
use simplex::simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplex::simplicityhl::elements::secp256k1_zkp::SECP256K1;

use simplex::signer::Signer;
use simplex::transaction::partial_input::IssuanceInput;
use simplex::transaction::{FinalTransaction, NativeSignature, PartialInput, PartialOutput};

fn make_confidential_to_bob(alice: &Signer, bob: &Signer, asset: AssetId) -> anyhow::Result<()> {
    let mut ft = FinalTransaction::new();

    ft.add_output(
        PartialOutput::new(bob.get_address().script_pubkey(), 1000, asset)
            .with_blinding_key(bob.get_blinding_public_key()),
    );

    let tx_receipt = alice.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

fn issue_confidential_to_alice(alice: &Signer, bob: &Signer) -> anyhow::Result<()> {
    let utxos = bob.get_utxos()?;

    let mut ft = FinalTransaction::new();

    let issuance_details = ft.add_issuance_input(
        PartialInput::new(utxos[0].clone()),
        IssuanceInput::new_issuance(1000, 100, [1u8; 32]),
        Some(NativeSignature::Ecdsa),
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

    Ok(())
}

#[simplex::test]
fn confidential_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let alice = context.get_default_signer();
    let bob = context.random_signer();

    make_confidential_to_bob(alice, &bob, provider.get_network().policy_asset())?;
    issue_confidential_to_alice(alice, &bob)?;

    // spend confidential
    let tx_receipt = bob.send(alice.get_address().script_pubkey(), 50)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

#[simplex::test]
fn output_keeps_its_chosen_blinding_factors(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let signer = context.get_default_signer();
    let asset_bf = AssetBlindingFactor::from_slice(&[0x01; 32])?;
    let value_bf = ValueBlindingFactor::from_slice(&[0x02; 32])?;

    let mut ft = FinalTransaction::new();

    // The change the signer adds balances the transaction
    ft.add_output(
        PartialOutput::new(
            signer.get_address().script_pubkey(),
            1000,
            provider.get_network().policy_asset(),
        )
        .with_blinding_key(signer.get_blinding_public_key())
        .with_asset_blinding_factor(asset_bf)
        .with_value_blinding_factor(value_bf),
    );

    let txid = signer.broadcast(&ft)?.txid();
    let secrets =
        provider.fetch_transaction(&txid)?.output[0].unblind(SECP256K1, signer.get_blinding_private_key().inner)?;

    assert_eq!((secrets.asset_bf, secrets.value_bf), (asset_bf, value_bf));

    Ok(())
}
