use simplex::simplicityhl::elements::{Script, Txid};

use simplex::constants::DUMMY_SIGNATURE;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

fn get_p2pk(context: &simplex::TestContext) -> (P2pkProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let p2pk = P2pkProgram::new(arguments);
    let p2pk_script = p2pk.get_script_pubkey(context.get_network());

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(context: &simplex::TestContext) -> anyhow::Result<Txid> {
    let signer = context.get_default_signer();

    let (_, p2pk_script) = get_p2pk(context);

    let txid = signer.send(p2pk_script.clone(), 50)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

fn spend_p2pk(context: &simplex::TestContext) -> anyhow::Result<Txid> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (p2pk, p2pk_script) = get_p2pk(context);

    let mut p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script)?;

    p2pk_utxos.retain(|utxo| utxo.explicit_asset() == context.get_network().policy_asset());

    let mut ft = FinalTransaction::new();

    let witness = P2pkWitness {
        signature: DUMMY_SIGNATURE,
    };

    ft.add_program_input(
        PartialInput::new(p2pk_utxos[0].clone()),
        ProgramInput::new(Box::new(p2pk.as_ref().clone()), Box::new(witness.clone())),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    let txid = signer.broadcast(&ft)?;
    println!("Broadcast: {}", txid);

    Ok(txid)
}

#[simplex::test]
fn basic_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let txid = spend_p2wpkh(&context)?;

    provider.wait(&txid)?;
    println!("Confirmed");

    let txid = spend_p2pk(&context)?;

    provider.wait(&txid)?;
    println!("Confirmed");

    Ok(())
}
