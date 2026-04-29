use simplex::simplicityhl::elements::{AssetId, Txid};

use simplex::signer::Signer;
use simplex::transaction::partial_input::IssuanceInput;
use simplex::transaction::{FinalTransaction, IssuanceDetails, PartialInput, PartialOutput, RequiredSignature};

fn make_confidential_to_bob(alice: &Signer, bob: &Signer, asset: AssetId) -> anyhow::Result<Txid> {
    let mut ft = FinalTransaction::new();

    ft.add_output(
        PartialOutput::new(bob.get_address().script_pubkey(), 1000, asset)
            .with_blinding_key(bob.get_blinding_public_key()),
    );

    let txid = alice.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

fn issue_explicit_to_alice_with_reissuance(alice: &Signer, bob: &Signer) -> anyhow::Result<(Txid, IssuanceDetails)> {
    let utxos = bob.get_utxos()?;

    let mut ft = FinalTransaction::new();

    let issuance_details = ft.add_issuance_input(
        PartialInput::new(utxos[0].clone()),
        IssuanceInput::new_issuance(1000, 100, [1u8; 32]),
        RequiredSignature::NativeEcdsa,
    );

    ft.add_output(PartialOutput::new(
        alice.get_address().script_pubkey(),
        1000,
        issuance_details.asset_id,
    ));
    ft.add_output(
        PartialOutput::new(
            bob.get_address().script_pubkey(),
            100,
            issuance_details.inflation_asset_id,
        )
        .with_blinding_key(bob.get_blinding_public_key()),
    );

    let txid = bob.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok((txid, issuance_details))
}

fn reissue_tokens_to_bob(
    bob: &Signer,
    issuance_details: &IssuanceDetails,
    reissuance_amount: u64,
) -> anyhow::Result<Txid> {
    let reissuance_token_utxo = bob.get_utxos_asset(issuance_details.inflation_asset_id)?[0].clone();

    let mut ft = FinalTransaction::new();

    ft.add_output(
        PartialOutput::new(
            bob.get_address().script_pubkey(),
            reissuance_token_utxo.unblinded_amount(),
            reissuance_token_utxo.unblinded_asset(),
        )
        .with_blinding_key(bob.get_blinding_public_key()),
    );

    ft.add_issuance_input(
        PartialInput::new(reissuance_token_utxo),
        IssuanceInput::new_reissuance(reissuance_amount, issuance_details.asset_entropy.0),
        RequiredSignature::NativeEcdsa,
    );

    ft.add_output(PartialOutput::new(
        bob.get_address().script_pubkey(),
        reissuance_amount,
        issuance_details.asset_id,
    ));

    let txid = bob.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

#[simplex::test]
fn reissuance_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let alice = context.get_default_signer();
    let bob = context.random_signer();

    let txid = make_confidential_to_bob(alice, &bob, provider.get_network().policy_asset())?;

    provider.wait(&txid)?;
    println!("Confirmed");

    let (txid, issuance_details) = issue_explicit_to_alice_with_reissuance(alice, &bob)?;

    provider.wait(&txid)?;
    println!("Confirmed");

    let reissuance_amount = 5000;
    let txid = reissue_tokens_to_bob(&bob, &issuance_details, reissuance_amount)?;
    println!("Broadcast: {}", txid);

    provider.wait(&txid)?;
    println!("Confirmed");

    let bob_asset_utxos = bob.get_utxos_asset(issuance_details.asset_id)?;

    assert!(bob_asset_utxos.len() == 1);
    assert!(bob_asset_utxos[0].explicit_amount() == reissuance_amount);

    Ok(())
}
