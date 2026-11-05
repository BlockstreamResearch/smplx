use simplex::constants::DUMMY_SIGNATURE;
use simplex::program::Program;
use simplex::signer::Signer;
use simplex::simplicityhl::WitnessValues;
use simplex::transaction::{FinalTransaction, PartialInput, PartialOutput, ProgramInput, ProgramSignature};

use simplex_fixtures::artifacts::nested_sig::NestedSigProgram;
use simplex_fixtures::artifacts::nested_sig::derived_nested_sig::{NestedSigArguments, NestedSigWitness};
use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

const FEE_RATE: f32 = 1000.0;

const LOCKED: u64 = 50_000;

/// Funds `program`, then spends the funds back to the signer at the fee the keyless estimate gives.
fn spend_at_strict_fee(
    context: &simplex::TestContext,
    program: &Program,
    witness: impl Into<WitnessValues>,
    signature: ProgramSignature,
) -> anyhow::Result<()> {
    let signer = context.get_default_signer();
    let provider = context.get_default_provider();
    let script = program.get_script_pubkey(context.get_network());
    let witness = witness.into();

    signer.send(script.clone(), LOCKED)?;

    let utxo = provider.fetch_scripthash_utxos(&script)?[0].clone();
    let spend = |fee: u64| {
        let mut ft = FinalTransaction::new();

        ft.add_program_input(
            PartialInput::new(utxo.clone()),
            ProgramInput::new(Box::new(program.clone()), witness.clone()),
            Some(signature.clone()),
        );
        ft.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            LOCKED - fee,
            context.get_network().policy_asset(),
        ));

        ft
    };

    let fee = Signer::estimate_strict_fee(&spend(0), context.get_network(), FEE_RATE)?;
    let (tx, paid) = signer.finalize_strict_fee(&spend(fee), FEE_RATE)?;

    // At 1000 sats/kvb, the fee is one sat per vbyte of the signed transaction
    assert_eq!(fee, u64::try_from(tx.discount_vsize())?);
    assert_eq!(paid, fee);

    // A larger surplus is paid as the fee, a smaller one is refused
    assert_eq!(signer.finalize_strict_fee(&spend(fee + 100), FEE_RATE)?.1, fee + 100);
    assert!(signer.finalize_strict_fee(&spend(fee - 1), FEE_RATE).is_err());

    provider.broadcast_transaction(&tx)?;

    Ok(())
}

#[simplex::test]
fn strict_fee_pays_a_p2pk_spend(context: simplex::TestContext) -> anyhow::Result<()> {
    let p2pk = P2pkProgram::new(P2pkArguments {
        public_key: context.get_default_signer().get_schnorr_public_key().serialize(),
    });

    spend_at_strict_fee(
        &context,
        p2pk.as_ref(),
        P2pkWitness::default(),
        ProgramSignature::new("SIGNATURE"),
    )
}

#[simplex::test]
fn strict_fee_pays_a_spend_with_pruned_branches(context: simplex::TestContext) -> anyhow::Result<()> {
    let nested_sig = NestedSigProgram::new(NestedSigArguments {
        public_key: context.get_default_signer().get_schnorr_public_key().serialize(),
    });
    let cold = NestedSigWitness {
        inherit_or_not: simplex::either::Either::Right(simplex::either::Either::Left(DUMMY_SIGNATURE)),
    };

    spend_at_strict_fee(
        &context,
        nested_sig.as_ref(),
        cold,
        ProgramSignature::new("INHERIT_OR_NOT").at_path(["Right", "Left"]),
    )
}
