use simplex::simplicityhl::elements::{Script, Txid};

use simplex::constants::DUMMY_SIGNATURE;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};
use simplex::utils::tr_unspendable_key;

use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

fn get_p2pk(context: &simplex::TestContext) -> (P2pkProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().unwrap().serialize(),
    };

    let p2pk = P2pkProgram::new(tr_unspendable_key(), arguments);
    let p2pk_script = p2pk.get_program().get_script_pubkey(context.get_network()).unwrap();

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(context: &simplex::TestContext) -> Txid {
    let signer = context.get_default_signer();

    let (_, p2pk_script) = get_p2pk(context);

    let res = signer.send(p2pk_script.clone(), 50).unwrap();

    println!("Broadcast: {}", res);

    res
}

fn spend_p2pk(context: &simplex::TestContext) -> Txid {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (p2pk, p2pk_script) = get_p2pk(context);

    let mut p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script).unwrap();

    p2pk_utxos.retain(|utxo| utxo.txout.asset.explicit().unwrap() == context.get_network().policy_asset());

    let mut ft = FinalTransaction::new();

    let witness = P2pkWitness {
        signature: DUMMY_SIGNATURE,
    };

    ft.add_program_input(
        PartialInput::new(p2pk_utxos[0].clone()),
        ProgramInput::new(Box::new(p2pk.get_program().clone()), Box::new(witness.clone())),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    )
    .unwrap();

    let res = signer.broadcast(&ft).unwrap();

    println!("Broadcast: {}", res);

    res
}

#[simplex::test]
fn basic_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let provider = context.get_default_provider();

    let tx = spend_p2wpkh(&context);
    provider.wait(&tx)?;

    println!("Confirmed");

    let tx = spend_p2pk(&context);
    provider.wait(&tx)?;

    println!("Confirmed");

    Ok(())
}
