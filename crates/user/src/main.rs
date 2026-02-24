use simplex_sdk::provider::provider::ProviderTrait;
use simplicityhl::elements::Script;

use simplex_sdk::presets::p2pk::P2PK;
use simplex_sdk::presets::p2pk::p2pk_build::{P2PKArguments, P2PKWitness};

use simplex_sdk::constants::{DUMMY_SIGNATURE, SimplicityNetwork};
use simplex_sdk::provider::esplora::EsploraProvider;
use simplex_sdk::signer::signer::Signer;
use simplex_sdk::transaction::final_transaction::FinalTransaction;
use simplex_sdk::transaction::partial_input::{PartialInput, ProgramInput, RequiredSignature};
use simplex_sdk::transaction::partial_output::PartialOutput;
use simplex_sdk::utils::tr_unspendable_key;

const ESPLORA_URL: &str = "https://blockstream.info/liquidtestnet/api";

fn get_p2pk(signer: &Signer) -> (P2PK, Script) {
    let arguments = P2PKArguments {
        public_key: signer.get_schnorr_public_key().unwrap().serialize(),
    };

    let p2pk = P2PK::new(tr_unspendable_key(), arguments);
    let p2pk_script = p2pk
        .get_program()
        .get_script_pubkey(SimplicityNetwork::LiquidTestnet)
        .unwrap();

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(signer: &Signer, provider: &EsploraProvider) {
    let mut signer_utxos = signer.get_wpkh_utxos().unwrap();

    for el in &signer_utxos {
        let outpoint = &el.0;
        let value = &el.1.value;
        let asset = &el.1.asset;
        let script = &el.1.script_pubkey;

        println!("Outpoint: {}", outpoint);
        println!("Value: {}", value);
        println!("Asset: {}", asset);
        println!("Script: {}", script);
    }

    signer_utxos.retain(|el| el.1.asset.explicit().unwrap() == SimplicityNetwork::LiquidTestnet.policy_asset());

    let (_, p2pk_script) = get_p2pk(&signer);

    let mut ft = FinalTransaction::new(SimplicityNetwork::LiquidTestnet);

    ft.add_input(
        PartialInput::new(signer_utxos[0].0, signer_utxos[0].1.clone()),
        RequiredSignature::NativeEcdsa,
    );
    ft.add_output(PartialOutput::new(
        p2pk_script.clone(),
        50,
        SimplicityNetwork::LiquidTestnet.policy_asset(),
    ));

    let (tx, _) = signer.finalize(&ft, 1).unwrap();
    let res = provider.broadcast_transaction(&tx).unwrap();

    println!("Broadcast: {}", res);
}

fn spend_p2pk(signer: &Signer, provider: &EsploraProvider) {
    let (p2pk, p2pk_script) = get_p2pk(&signer);

    let mut p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script).unwrap();

    for el in &p2pk_utxos {
        let outpoint = el.0;

        println!("{outpoint}");
    }

    p2pk_utxos.retain(|el| el.1.asset.explicit().unwrap() == SimplicityNetwork::LiquidTestnet.policy_asset());

    let mut ft = FinalTransaction::new(SimplicityNetwork::LiquidTestnet);

    let witness = P2PKWitness {
        signature: DUMMY_SIGNATURE,
    };

    ft.add_program_input(
        PartialInput::new(p2pk_utxos[0].0, p2pk_utxos[0].1.clone()),
        ProgramInput::new(Box::new(p2pk.get_program().clone()), Box::new(witness.clone())),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );
    ft.add_output(PartialOutput::new(
        signer.get_wpkh_address().unwrap().script_pubkey(),
        20,
        SimplicityNetwork::LiquidTestnet.policy_asset(),
    ));

    let (tx, _) = signer.finalize(&ft, 1).unwrap();
    let res = provider.broadcast_transaction(&tx).unwrap();

    println!("Broadcast: {}", res);
}

fn main() {
    let provider = EsploraProvider::new(ESPLORA_URL.to_string());
    let signer = Signer::new(
        "exist carry drive collect lend cereal occur much tiger just involve mean",
        Box::new(provider.clone()),
        SimplicityNetwork::LiquidTestnet,
    )
    .unwrap();

    spend_p2wpkh(&signer, &provider);
    // TODO: wait for confirmation
    spend_p2pk(&signer, &provider);

    println!("OK");
}
