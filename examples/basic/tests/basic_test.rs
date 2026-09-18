use simplex::simplicityhl::elements::Script;

use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature, TxReceipt};

use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

fn get_p2pk(context: &simplex::TestContext) -> (P2pkProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = P2pkArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let p2pk = P2pkProgram::new(&arguments);
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
        ProgramInput::new(Box::new(p2pk.as_ref().clone()), Box::new(witness.clone())),
        RequiredSignature::Witness("SIGNATURE".to_string()),
    );

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(tx_receipt)
}

#[simplex::test]
fn basic_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let (program, _) = get_p2pk(&context);
    let mut program = program
        .with_taproot_pubkey(context.get_default_signer().get_schnorr_public_key())
        .with_storage_capacity(1);
    program.set_storage_at(0, [7u8; 32]);
    assert_eq!(program.get_storage_len(), 1);
    assert_eq!(program.get_storage_at(0), vec![7u8; 32]);
    assert_eq!(program.get_storage(), &[vec![7u8; 32]]);
    let arguments = program.as_ref().get_argument_types()?;
    let arguments: Vec<_> = arguments
        .iter()
        .map(|(name, ty)| (name.to_string(), ty.to_string()))
        .collect();
    assert_eq!(arguments, [("PUBLIC_KEY".to_string(), "u256".to_string())]);
    let witnesses = program.as_ref().get_witness_types()?;
    let witnesses: Vec<_> = witnesses
        .iter()
        .map(|(name, ty)| (name.to_string(), ty.to_string()))
        .collect();
    assert_eq!(witnesses, [("SIGNATURE".to_string(), "[u8; 64]".to_string())]);
    assert_eq!(
        program.get_script_pubkey(context.get_network()),
        program.as_ref().get_tr_address(context.get_network()).script_pubkey()
    );
    assert_eq!(
        program.get_script_hash(context.get_network()),
        simplex::utils::hash_script(&program.get_script_pubkey(context.get_network()))
    );
    let _ = program.get_cmr();
    let _ = program.get_tapleaf_hash();

    let network_utils = context.get_network_utils();
    let current_height = context.get_default_provider().fetch_tip_height()? as u64;
    network_utils.mine_until_height(current_height)?;
    assert_eq!(
        context.get_default_provider().fetch_tip_height()? as u64,
        current_height
    );
    network_utils.mine_until_height(current_height + 1)?;
    assert!(context.get_default_provider().fetch_tip_height()? as u64 >= current_height + 1);

    let tx_receipt = spend_p2wpkh(&context)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    let tx_receipt = spend_p2pk(&context)?;

    tx_receipt.wait()?;
    println!("Confirmed");

    Ok(())
}
