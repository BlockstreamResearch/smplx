use simplex::simplex_macros::*;
use simplex::simplex_sdk::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui/option_offer.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_option_offer::OptionOfferWitness { path: simplex::either::Left((0, false)) };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_option_offer::OptionOfferWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_option_offer::OptionOfferArguments {
        user_pubkey: [1; 32],
        premium_per_collateral: 0,
        premium_asset_id: [1; 32],
        settlement_asset_id: [1; 32],
        collateral_asset_id: [1; 32],
        collateral_per_contract: 0,
        expiry_time: 0,
    };

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_option_offer::OptionOfferArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}
