use simplicityhl::elements::{Script, Txid};

use simplex::simplex_sdk::constants::DUMMY_SIGNATURE;
use simplex::simplex_sdk::provider::{EsploraProvider, ProviderTrait, SimplicityNetwork};
use simplex::simplex_sdk::signer::Signer;
use simplex::simplex_sdk::transaction::{
    FinalTransaction, PartialInput, PartialOutput, ProgramInput, RequiredSignature,
};
use simplex::simplex_sdk::utils::tr_unspendable_key;

use draft_example::artifacts::p2pk::P2pkProgram;
use draft_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

const ESPLORA_URL: &str = "https://blockstream.info/liquidtestnet/api";

fn get_p2pk(signer: &Signer) -> (P2pkProgram, Script) {
    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().unwrap().serialize(),
    };

    let p2pk = P2pkProgram::new(tr_unspendable_key(), arguments);
    let p2pk_script = p2pk
        .get_program()
        .get_script_pubkey(SimplicityNetwork::LiquidTestnet)
        .unwrap();

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(signer: &Signer, provider: &EsploraProvider) -> Txid {
    let (_, p2pk_script) = get_p2pk(signer);

    let mut ft = FinalTransaction::new(SimplicityNetwork::LiquidTestnet);

    ft.add_output(PartialOutput::new(
        p2pk_script.clone(),
        50,
        SimplicityNetwork::LiquidTestnet.policy_asset(),
    ));

    let (tx, _) = signer.finalize(&ft, 1).unwrap();
    let res = provider.broadcast_transaction(&tx).unwrap();

    println!("Broadcast: {}", res);

    res
}

fn spend_p2pk(signer: &Signer, provider: &EsploraProvider) -> Txid {
    let (p2pk, p2pk_script) = get_p2pk(signer);

    let mut p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script).unwrap();

    p2pk_utxos.retain(|el| el.1.asset.explicit().unwrap() == SimplicityNetwork::LiquidTestnet.policy_asset());

    let mut ft = FinalTransaction::new(SimplicityNetwork::LiquidTestnet);

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

fn main() -> anyhow::Result<()> {
    let provider = EsploraProvider::new(ESPLORA_URL.to_string());
    let signer = Signer::new(
        "exist carry drive collect lend cereal occur much tiger just involve mean",
        provider.clone(),
        SimplicityNetwork::LiquidTestnet,
    )?;

    let tx = spend_p2wpkh(&signer, &provider);

    provider.wait(&tx)?;

    println!("Confirmed");

    let tx = spend_p2pk(&signer, &provider);

    provider.wait(&tx)?;

    println!("Confirmed");

    println!("OK");

    Ok(())
}
