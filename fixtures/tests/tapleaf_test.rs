use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_fixtures::artifacts::tapleaf_check::TapleafCheckProgram;
use simplex_fixtures::artifacts::tapleaf_check::derived_tapleaf_check::{TapleafCheckArguments, TapleafCheckWitness};

#[simplex::test]
fn program_tapleaf_test(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let tapleaf_check = TapleafCheckProgram::new(TapleafCheckArguments::default());
    let tapleaf_check_script = tapleaf_check.get_script_pubkey(context.get_network());

    let tx_receipt = signer.send(tapleaf_check_script.clone(), 50)?;
    tx_receipt.wait()?;

    let tapleaf_check_utxo = provider.fetch_scripthash_utxos(&tapleaf_check_script)?[0].clone();

    let mut ft = FinalTransaction::new();

    let witness = TapleafCheckWitness {
        program_tapleaf_hash: tapleaf_check.get_tapleaf_hash(),
    };

    ft.add_program_input(
        PartialInput::new(tapleaf_check_utxo),
        ProgramInput::new(Box::new(tapleaf_check.as_ref().clone()), witness),
        RequiredSignature::None,
    );

    let tx_receipt = signer.broadcast(&ft)?;
    tx_receipt.wait()?;

    Ok(())
}
