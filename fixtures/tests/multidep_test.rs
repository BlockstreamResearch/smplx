use simplex::simplicityhl::elements::Script;

use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_fixtures::artifacts::imports::multidep::MultidepProgram;
use simplex_fixtures::artifacts::imports::multidep::derived_multidep::{MultidepArguments, MultidepWitness};

fn get_multidep(context: &simplex::TestContext) -> (MultidepProgram, Script) {
    let arguments = MultidepArguments { prev_hash: 5 };

    let p2pk = MultidepProgram::new(arguments);
    let p2pk_script = p2pk.get_script_pubkey(context.get_network());

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(context: &simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let (_, script) = get_multidep(context);

    let tx_receipt = signer.send(script.clone(), 50)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

fn spend_p2pk(context: &simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (program, script) = get_multidep(context);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    let witness = MultidepWitness { tx1: 15 };

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(program.as_ref().clone()), witness),
        RequiredSignature::None,
    );

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

#[simplex::test]
fn multidep_test(context: simplex::TestContext) -> anyhow::Result<()> {
    spend_p2wpkh(&context)?;
    spend_p2pk(&context)?;

    Ok(())
}
