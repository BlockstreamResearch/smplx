use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/option_offer.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_option_offer::OptionOfferWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_option_offer::OptionOfferWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_option_offer::OptionOfferArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_option_offer::OptionOfferArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}
