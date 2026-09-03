use std::sync::Arc;

use simplex::constants::DUMMY_SIGNATURE;
use simplex::either::Either;
use simplex::simplicityhl::elements::Script;
use simplex::simplicityhl::simplicity::hashes::{Hash, sha256};
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, SigMessage};

use simplex_fixtures::artifacts::sig_message::SigMessageProgram;
use simplex_fixtures::artifacts::sig_message::derived_sig_message::{SigMessageArguments, SigMessageWitness};

/// Must match the tag hashed into `tag_hash()` in sig_message.simf.
const TAG: &str = "SimplexFixture/SigMessage";

fn get_sig_message(context: &simplex::TestContext) -> (SigMessageProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = SigMessageArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let program = SigMessageProgram::new(&arguments);
    let script = program.get_script_pubkey(context.get_network());

    (program, script)
}

fn fund_sig_message(context: &simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let (_, script) = get_sig_message(context);

    let tx_receipt = signer.send(script, 50_000)?;
    println!("Funded: {}", tx_receipt);

    Ok(())
}

fn spend_sig_message(
    context: &simplex::TestContext,
    witness: SigMessageWitness,
    required_sig: RequiredSignature,
) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (program, script) = get_sig_message(context);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(program.as_ref().clone()), Box::new(witness)),
        required_sig,
    );

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

/// The program checks the signature against `jet::sig_all_hash()` directly.
#[simplex::test]
fn test_sighash_message(context: simplex::TestContext) -> anyhow::Result<()> {
    fund_sig_message(&context)?;

    let witness = SigMessageWitness {
        signature: Either::Left(DUMMY_SIGNATURE),
    };

    spend_sig_message(
        &context,
        witness,
        RequiredSignature::witness_with_path("SIGNATURE", ["Left"]),
    )
}

/// The program checks the signature against `sha256(tag || tag || sig_all_hash)`.
#[simplex::test]
fn test_tagged_message(context: simplex::TestContext) -> anyhow::Result<()> {
    fund_sig_message(&context)?;

    let witness = SigMessageWitness {
        signature: Either::Right(Either::Left(DUMMY_SIGNATURE)),
    };

    spend_sig_message(
        &context,
        witness,
        RequiredSignature::witness_tagged("SIGNATURE", ["Right", "Left"], TAG),
    )
}

/// The program checks the signature against `sha256(sig_all_hash)`, which is neither
/// the sighash nor a tagged hash, so it can only be expressed as a closure.
#[simplex::test]
fn test_custom_message(context: simplex::TestContext) -> anyhow::Result<()> {
    fund_sig_message(&context)?;

    let witness = SigMessageWitness {
        signature: Either::Right(Either::Right(DUMMY_SIGNATURE)),
    };

    let message = SigMessage::Custom(Arc::new(|sighash: [u8; 32]| {
        sha256::Hash::hash(&sighash).to_byte_array()
    }));

    spend_sig_message(
        &context,
        witness,
        RequiredSignature::witness_with_message("SIGNATURE", ["Right", "Right"], message),
    )
}
