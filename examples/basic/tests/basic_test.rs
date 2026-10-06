use simplex::simplicityhl::Arguments;
use simplex::simplicityhl::elements::Script;

use simplex::fuzz::core::Expect;
use simplex::fuzz::engine::FuzzStrategyBuilder;
use simplex::fuzz::proptest::strategy::Strategy;
use simplex::fuzz::transaction::FuzzTransaction;
use simplex::provider::SimplicityNetwork;
use simplex::signer::Signer;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, TxReceipt};
use simplex::{FuzzMode, TestContext};

use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

fn get_p2pk_arguments(signer: &Signer) -> P2pkArguments {
    P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    }
}

fn get_p2pk(context: &simplex::TestContext) -> (P2pkProgram, Script) {
    let arguments = get_p2pk_arguments(context.get_default_signer());

    let p2pk = P2pkProgram::new(arguments);
    let p2pk_script = p2pk.get_script_pubkey(context.get_network());

    (p2pk, p2pk_script)
}

fn spend_p2wpkh(context: &simplex::TestContext) -> anyhow::Result<TxReceipt<'_>> {
    let signer = context.get_default_signer();

    let (_, p2pk_script) = get_p2pk(context);

    let tx_receipt = signer.send(p2pk_script.clone(), 50)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

fn spend_p2pk(context: &simplex::TestContext) -> anyhow::Result<TxReceipt<'_>> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (p2pk, p2pk_script) = get_p2pk(context);

    let p2pk_utxos = provider.fetch_scripthash_utxos(&p2pk_script)?;

    let mut ft = FinalTransaction::new();

    let witness = P2pkWitness::default();

    ft.add_program_input(
        PartialInput::new(p2pk_utxos[0].clone()),
        ProgramInput::new(Box::new(p2pk.as_ref().clone()), witness.clone()),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

#[simplex::test]
fn basic_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let tx_receipt = spend_p2wpkh(&context)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    let tx_receipt = spend_p2pk(&context)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    Ok(())
}

#[simplex::fuzz]
fn p2pk_fuzz_test(mut context: TestContext<FuzzMode>) -> anyhow::Result<()> {
    const TEST_MNEMONIC: &str = "exist carry drive collect lend cereal occur much tiger just involve mean";

    let signer = Signer::from_mnemonic(TEST_MNEMONIC, SimplicityNetwork::default_regtest());
    let arguments = get_p2pk_arguments(&signer);
    context.set_custom_signer(signer);

    // Keep the signer's public key fixed while fuzzing the initial witness.
    let fixed_arguments: Arguments = (&arguments).into();
    let strategy = FuzzStrategyBuilder::<P2pkArguments, P2pkWitness>::new()
        .build()
        .prop_map(move |(_, witness)| (fixed_arguments.clone(), witness));

    let blueprint = FuzzTransaction::try_default()?.with_post_hook(|transaction, _, _, _| {
        // Ask the signer to replace each fuzzed `SIGNATURE` with a valid signature.
        transaction.inputs_mut()[0].required_sig = RequiredSignature::Witness("SIGNATURE".to_string());
        Ok(())
    });

    let runner = context.build::<P2pkProgram, P2pkArguments, P2pkWitness>(strategy, blueprint);

    runner.run_with_default_check(Expect::Ok);

    Ok(())
}
