use simplex::constants::DUMMY_SIGNATURE;
use simplex::either::Either;
use simplex::program::ProgramError;
use simplex::signer::SignerError;
use simplex::simplicityhl::elements::Script;
use simplex::transaction::{FinalTransaction, PartialInput, ProgramInput, RequiredSignature};

use simplex_fixtures::artifacts::budget::BudgetProgram;
use simplex_fixtures::artifacts::budget::derived_budget::{BudgetArguments, BudgetWitness};

fn get_budget(context: &simplex::TestContext) -> (BudgetProgram, Script) {
    let signer = context.get_default_signer();

    let arguments = BudgetArguments {
        public_key: signer.get_schnorr_public_key().serialize(),
    };

    let program = BudgetProgram::new(arguments);
    let script = program.get_script_pubkey(context.get_network());

    (program, script)
}

/// Funds the budget program and assembles a spend of it along `sig_path`.
fn spend_budget(
    context: &simplex::TestContext,
    witness: BudgetWitness,
    sig_path: &str,
) -> anyhow::Result<FinalTransaction> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();

    let (program, script) = get_budget(context);

    let tx_receipt = signer.send(script.clone(), 50_000)?;
    println!("Funded: {}", tx_receipt);

    let utxos = provider.fetch_scripthash_utxos(&script)?;

    let mut ft = FinalTransaction::new();

    ft.add_program_input(
        PartialInput::new(utxos[0].clone()),
        ProgramInput::new(Box::new(program.as_ref().clone()), witness),
        RequiredSignature::witness_with_path("SIGNATURE", [sig_path]),
    );

    Ok(ft)
}

fn assert_insufficient_budget<T>(result: Result<T, SignerError>) {
    match result {
        Err(SignerError::CovenantExecution {
            index: 0,
            source:
                ProgramError::InsufficientBudget {
                    cost_wu,
                    budget_wu,
                    stack_bytes,
                    deficit_wu,
                },
            ..
        }) => {
            assert!(deficit_wu > 0);
            assert_eq!(cost_wu, budget_wu + deficit_wu);
            assert_eq!(budget_wu, stack_bytes as u64 + 50);
        }
        Err(other) => panic!("expected an insufficient budget error, got {other:?}"),
        Ok(_) => panic!("expected an insufficient budget error, but the spend went through"),
    }
}

/// The node accepts a program whose cost fits the budget, so the local check is not stricter than consensus.
#[simplex::test]
fn test_within_budget_is_accepted(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let witness = BudgetWitness {
        signature: Either::Left(DUMMY_SIGNATURE),
    };

    let ft = spend_budget(&context, witness, "Left")?;

    let tx_receipt = signer.broadcast(&ft)?;
    println!("Broadcast: {}", tx_receipt);

    Ok(())
}

/// The signer rejects an over-budget program before the transaction reaches the node.
#[simplex::test]
fn test_over_budget_is_rejected_before_broadcast(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let witness = BudgetWitness {
        signature: Either::Right(DUMMY_SIGNATURE),
    };

    let ft = spend_budget(&context, witness, "Right")?;

    assert_insufficient_budget(signer.broadcast(&ft));

    Ok(())
}

#[simplex::test]
fn test_fee_estimation_reports_over_budget(context: simplex::TestContext) -> anyhow::Result<()> {
    let signer = context.get_default_signer();

    let witness = BudgetWitness {
        signature: Either::Right(DUMMY_SIGNATURE),
    };

    let ft = spend_budget(&context, witness, "Right")?;

    assert_insufficient_budget(signer.estimate_fee(&ft, 100.0));

    Ok(())
}
