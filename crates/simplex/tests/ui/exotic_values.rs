use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/exotic_values.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_exotic_values::ExoticValuesWitness {
        path: simplex::either::Left((simplex::either::Left(1), simplex::either::Left(Some(false)))),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_exotic_values::ExoticValuesWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_exotic_values::ExoticValuesArguments {};

    let witness_values = original_arguments.build_arguments();
    let recovered_witness =
        derived_exotic_values::ExoticValuesArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}
