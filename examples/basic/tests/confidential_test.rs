use simplex::simplicityhl::elements::{AssetId, Txid};

use simplex::signer::Signer;
use simplex::transaction::partial_input::IssuanceInput;
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature};

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

fn issue_confidential_to_alice(alice: &Signer, bob: &Signer) -> anyhow::Result<Txid> {
    let utxos = bob.get_utxos()?;

    let mut ft = FinalTransaction::new();

    let (issuance_id, reissuance_id) = ft.add_issuance_input(
        PartialInput::new(utxos[0].clone()),
        IssuanceInput::new(1000, [1u8; 32])
            .with_reissuance(100)
            .with_blinding_nonce([1u8; 32]),
        RequiredSignature::NativeEcdsa,
    );

    ft.add_output(
        PartialOutput::new(alice.get_address().script_pubkey(), 1000, issuance_id)
            .with_blinding_key(alice.get_blinding_public_key()),
    );
    ft.add_output(
        PartialOutput::new(alice.get_address().script_pubkey(), 100, reissuance_id)
            .with_blinding_key(alice.get_blinding_public_key()),
    );

    let txid = bob.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

#[simplex::test]
fn confidential_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let alice = context.get_default_signer();
    let bob = context.random_signer();

    let txid = make_confidential_to_bob(&alice, &bob, provider.get_network().policy_asset())?;

    provider.wait(&txid)?;
    println!("Confirmed");

    let txid = issue_confidential_to_alice(&alice, &bob)?;

    provider.wait(&txid)?;
    println!("Confirmed");

    // spend confidential
    let txid = bob.send(alice.get_address().script_pubkey(), 50)?;
    println!("Broadcast: {}", txid);

    provider.wait(&txid)?;
    println!("Confirmed");

    Ok(())
}
