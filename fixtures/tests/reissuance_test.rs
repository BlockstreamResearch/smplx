use simplex::simplicityhl::elements::confidential::AssetBlindingFactor;
use simplex::simplicityhl::elements::secp256k1_zkp::SECP256K1;
use simplex::simplicityhl::elements::{AssetId, OutPoint, Script, Txid};

use simplex::signer::Signer;
use simplex::transaction::partial_input::IssuanceInput;
use simplex::transaction::{
    FinalTransaction, IssuanceDetails, NativeSignature, PartialInput, PartialOutput, ProgramInput, ProgramSignature,
    TxReceipt, UTXO,
};

use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

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

fn issue_explicit_to_alice_with_reissuance<'a>(
    alice: &Signer,
    bob: &'a Signer,
) -> anyhow::Result<(TxReceipt<'a>, IssuanceDetails)> {
    let utxos = bob.get_utxos()?;

    let mut ft = FinalTransaction::new();

    let issuance_details = ft.add_issuance_input(
        PartialInput::new(utxos[0].clone()),
        IssuanceInput::new_issuance(1000, 100, [1u8; 32]),
        Some(NativeSignature::Ecdsa),
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

    let tx_receipt = bob.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok((tx_receipt, issuance_details))
}

fn reissue_tokens_to_bob<'a>(
    bob: &'a Signer,
    issuance_details: &IssuanceDetails,
    reissuance_amount: u64,
) -> anyhow::Result<TxReceipt<'a>> {
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
        IssuanceInput::new_reissuance(reissuance_amount, issuance_details.asset_entropy.to_byte_array()),
        Some(NativeSignature::Ecdsa),
    );

    ft.add_output(PartialOutput::new(
        bob.get_address().script_pubkey(),
        reissuance_amount,
        issuance_details.asset_id,
    ));

    let tx_receipt = bob.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

#[simplex::test]
fn reissuance_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let alice = context.get_default_signer();
    let bob = context.random_signer();

    let tx_receipt = make_confidential_to_bob(alice, &bob, provider.get_network().policy_asset())?;

    tx_receipt.wait()?;
    println!("Confirmed");

    let (tx_receipt, issuance_details) = issue_explicit_to_alice_with_reissuance(alice, &bob)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    let reissuance_amount = 5000;
    let tx_receipt = reissue_tokens_to_bob(&bob, &issuance_details, reissuance_amount)?;
    println!("Broadcast: {}", tx_receipt);

    tx_receipt.wait()?;
    println!("Confirmed");

    let bob_asset_utxos = bob.get_utxos_asset(issuance_details.asset_id)?;

    assert_eq!(bob_asset_utxos.len(), 1);
    assert_eq!(bob_asset_utxos[0].explicit_amount(), reissuance_amount);

    Ok(())
}

/// The asset blinding factor `n`.
fn factor(n: u8) -> AssetBlindingFactor {
    let mut bytes = [0; 32];
    bytes[31] = n;

    AssetBlindingFactor::from_slice(&bytes).expect("a small scalar")
}

/// The reissuance token at output 1 of `txid`, unblinded with the signer's blinding key.
fn token_of(context: &simplex::TestContext, txid: Txid) -> anyhow::Result<UTXO> {
    let txout = context.get_default_provider().fetch_transaction(&txid)?.output[1].clone();
    let secrets = txout.unblind(SECP256K1, context.get_default_signer().get_blinding_private_key().inner)?;

    Ok(UTXO {
        outpoint: OutPoint::new(txid, 1),
        txout,
        secrets: Some(secrets),
    })
}

/// Reissues 500 of the asset from the token `program` holds, and locks the token back blinded
/// with `next_asset_bf`, at the fee the estimate gives. Returns the reissuance's txid.
fn reissue_from_program(
    context: &simplex::TestContext,
    program: &P2pkProgram,
    token: UTXO,
    issuance_details: &IssuanceDetails,
    next_asset_bf: AssetBlindingFactor,
) -> anyhow::Result<Txid> {
    let signer = context.get_default_signer();
    let script = program.get_script_pubkey(context.get_network());
    let mut ft = FinalTransaction::new();

    ft.add_program_issuance_input(
        PartialInput::new(token),
        ProgramInput::new(Box::new(program.as_ref().clone()), P2pkWitness::default()),
        IssuanceInput::new_reissuance(500, issuance_details.asset_entropy.to_byte_array()),
        Some(ProgramSignature::new("SIGNATURE")),
    );
    ft.add_output(PartialOutput::new(
        signer.get_address().script_pubkey(),
        500,
        issuance_details.asset_id,
    ));
    ft.add_output(locked_token(signer, script, issuance_details, next_asset_bf));

    let fee = signer.estimate(&ft)?;
    let (tx, paid) = signer.finalize(&ft)?;

    assert_eq!(paid, fee);

    Ok(context.get_default_provider().broadcast_transaction(&tx)?.txid())
}

/// The reissuance token locked to `script`, blinded with `asset_bf`.
fn locked_token(
    signer: &Signer,
    script: Script,
    issuance_details: &IssuanceDetails,
    asset_bf: AssetBlindingFactor,
) -> PartialOutput {
    PartialOutput::new(script, 1, issuance_details.inflation_asset_id)
        .with_blinding_key(signer.get_blinding_public_key())
        .with_asset_blinding_factor(asset_bf)
}

#[simplex::test]
fn reissuance_from_a_token_a_program_holds(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let p2pk = P2pkProgram::new(P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    });

    // Issue the asset and lock its token to the program, blinded with factor 1
    let mut ft = FinalTransaction::new();
    let issuance_details = ft.add_issuance_input(
        PartialInput::new(signer.get_utxos()?[0].clone()),
        IssuanceInput::new_issuance(1000, 1, [2u8; 32]),
        Some(NativeSignature::Ecdsa),
    );

    ft.add_output(PartialOutput::new(
        signer.get_address().script_pubkey(),
        1000,
        issuance_details.asset_id,
    ));
    ft.add_output(locked_token(
        signer,
        p2pk.get_script_pubkey(context.get_network()),
        &issuance_details,
        factor(1),
    ));

    let mut txid = signer.broadcast(&ft)?.txid();

    // Each reissuance spends the token with the factor it was locked with and advances the factor
    for n in 1..=2 {
        let token = token_of(&context, txid)?;

        assert_eq!(token.secrets.map(|secrets| secrets.asset_bf), Some(factor(n)));

        txid = reissue_from_program(&context, &p2pk, token, &issuance_details, factor(n + 1))?;
    }

    Ok(())
}
