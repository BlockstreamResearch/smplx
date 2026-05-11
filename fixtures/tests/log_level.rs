use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_fixtures::artifacts::dummy_panic::DummyPanicProgram;
use simplex_fixtures::artifacts::dummy_panic::derived_dummy_panic::{DummyPanicArguments, DummyPanicWitness};

fn setup_dummy(context: &simplex::TestContext) -> (DummyPanicProgram, simplex::simplicityhl::elements::Script) {
    let signer = context.get_default_signer();

    let dummy = DummyPanicProgram::new(DummyPanicArguments {}).with_taproot_pubkey(signer.get_schnorr_public_key());

    let script = dummy.get_script_pubkey(context.get_network());

    (dummy, script)
}

#[simplex::test]
fn dummy_log_level(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();
    let signer = context.get_default_signer();

    let (dummy, script) = setup_dummy(&context);

    let tx_receipt = signer.send(script.clone(), 50)?;
    println!("Funded dummy script: {}", tx_receipt);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(dummy.as_ref().clone()), Box::new(DummyPanicWitness {})),
        RequiredSignature::None,
    );

    let result = signer.broadcast(&ft);

    assert!(result.is_err(), "expected assert!(false) program to fail execution");
    println!("{}", result.err().unwrap());

    Ok(())
}
