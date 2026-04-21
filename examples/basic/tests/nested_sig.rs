use simplex::constants::DUMMY_SIGNATURE;
use simplex::simplicityhl::elements::{Script, Txid};
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_example::artifacts::nested_sig::NestedSigProgram;
use simplex_example::artifacts::nested_sig::derived_nested_sig::{NestedSigArguments, NestedSigWitness};

fn get_nested_sig(context: &simplex::TestContext) -> (NestedSigProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = NestedSigArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let program = NestedSigProgram::new(arguments);
    let script = program.get_script_pubkey(context.get_network());

    (program, script)
}

fn fund_nested_sig(context: &simplex::TestContext) -> anyhow::Result<Txid> {
    let signer = context.get_default_signer();
    let (_, script) = get_nested_sig(context);

    let txid = signer.send(script, 50_000)?;
    println!("Funded: {}", txid);

    Ok(txid)
}

fn spend_nested_sig(
    context: &simplex::TestContext,
    witness: NestedSigWitness,
    sig_path: &[&str],
) -> anyhow::Result<Txid> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (program, script) = get_nested_sig(context);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(program.as_ref().clone()), Box::new(witness)),
        RequiredSignature::witness_with_path("INHERIT_OR_NOT", sig_path),
    );

    let txid = signer.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

#[simplex::test]
fn test_inherit_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let fund_tx = fund_nested_sig(&context)?;
    provider.wait(&fund_tx)?;

    // Left - inheritor sig injected by signer at path L
    let witness = NestedSigWitness {
        inherit_or_not: simplex::either::Either::Left((DUMMY_SIGNATURE, [0; 32])),
    };

    let spend_tx = spend_nested_sig(&context, witness, &["Left", "0"])?;
    provider.wait(&spend_tx)?;
    println!("Inherit spend confirmed");

    Ok(())
}

#[simplex::test]
fn test_cold_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let fund_tx = fund_nested_sig(&context)?;
    provider.wait(&fund_tx)?;

    // Right Left - cold sig injected by signer at path R L
    let witness = NestedSigWitness {
        inherit_or_not: simplex::either::Either::Right(simplex::either::Either::Left(DUMMY_SIGNATURE)),
    };

    let spend_tx = spend_nested_sig(&context, witness, &["Right", "Left"])?;
    provider.wait(&spend_tx)?;
    println!("Cold spend confirmed");

    Ok(())
}

#[simplex::test]
fn test_hot_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let fund_tx = fund_nested_sig(&context)?;
    provider.wait(&fund_tx)?;

    // Right Right - hot sig injected by signer at path R R
    let witness = NestedSigWitness {
        inherit_or_not: simplex::either::Either::Right(simplex::either::Either::Right([DUMMY_SIGNATURE, [0; 64]])),
    };

    let spend_tx = spend_nested_sig(&context, witness, &["Right", "Right", "0"])?;
    provider.wait(&spend_tx)?;
    println!("Hot spend confirmed");

    Ok(())
}
