use simplicityhl::elements::{Script, Txid};

use simplex::simplex_sdk::constants::DUMMY_SIGNATURE;
use simplex::simplex_sdk::provider::ProviderTrait;
use simplex::simplex_sdk::signer::Signer;
use simplex::simplex_sdk::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex::simplex_sdk::utils::tr_unspendable_key;

use draft_example::artifacts::p2pk::P2pkProgram;
use draft_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

fn get_p2pk(context: &simplex::TestContext, signer: &Signer) -> (P2pkProgram, Script) {
    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().unwrap().serialize(),
    };

    let p2pk = P2pkProgram::new(tr_unspendable_key(), arguments);
    let p2pk_script = p2pk.get_program().get_script_pubkey(*context.get_network()).unwrap();

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(context: &simplex::TestContext, signer: &Signer, provider: &dyn ProviderTrait) -> Txid {
    let (_, p2pk_script) = get_p2pk(context, signer);

    let mut ft = FinalTransaction::new(*context.get_network());

    ft.add_output(PartialOutput::new(
        p2pk_script.clone(),
        50,
        context.get_network().policy_asset(),
    ));

    let (tx, _) = signer.finalize(&ft, 1).unwrap();
    let res = provider.broadcast_transaction(&tx).unwrap();

    println!("Broadcast: {}", res);

    res
}

fn spend_p2pk(context: &simplex::TestContext, signer: &Signer, provider: &dyn ProviderTrait) -> Txid {
    let (p2pk, p2pk_script) = get_p2pk(context, signer);

    let mut p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script).unwrap();

    p2pk_utxos.retain(|el| el.1.asset.explicit().unwrap() == context.get_network().policy_asset());

    let mut ft = FinalTransaction::new(*context.get_network());

    let witness = P2pkWitness {
        signature: DUMMY_SIGNATURE,
    };

    ft.add_program_input(
        PartialInput::new(p2pk_utxos[0].0, p2pk_utxos[0].1.clone()),
        ProgramInput::new(Box::new(p2pk.get_program().clone()), Box::new(witness.clone())),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    )
    .unwrap();

    let (tx, _) = signer.finalize(&ft, 1).unwrap();
    let res = provider.broadcast_transaction(&tx).unwrap();

    println!("Broadcast: {}", res);

    res
}

#[simplex::test]
fn dummy_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_signer();
    let provider = context.get_provider();

    let tx = spend_p2wpkh(&context, &signer, provider.as_ref());

    provider.wait(&tx)?;

    println!("Confirmed");

    let tx = spend_p2pk(&context, &signer, provider.as_ref());

    provider.wait(&tx)?;

    println!("Confirmed");

    println!("OK");

    Ok(())
}
