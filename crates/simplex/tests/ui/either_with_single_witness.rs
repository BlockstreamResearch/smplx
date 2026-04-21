use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/either_with_single_witness.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_either_with_single_witness::EitherWithSingleWitnessWitness {
        path: simplex::either::Left((simplex::either::Left(11_u16), simplex::either::Left(Some(true)))),
        signature: [0; 64],
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_either_with_single_witness::EitherWithSingleWitnessArguments {};

    let witness_values = original_arguments.build_arguments();
    let recovered_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}
