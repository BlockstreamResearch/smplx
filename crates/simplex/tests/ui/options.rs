use simplex_macros::*;
include_simf!("../../../../crates/simplex/tests/ui/options.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_options::OptionsWitness {
        path: simplicityhl::either::Either::Right(simplicityhl::either::Either::Left((true, 100, 200))),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_options::OptionsWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_options::OptionsArguments {
        start_time: 0,
        expiry_time: 0,
        grantor_reissuance_token_asset: Default::default(),
        grantor_token_asset: Default::default(),
        settlement_per_contract: Default::default(),
        settlement_asset_id: Default::default(),
        collateral_per_contract: Default::default(),
        collateral_asset_id: Default::default(),
        option_reissuance_token_asset: Default::default(),
        option_token_asset: Default::default(),
    };

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_options::OptionsArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}