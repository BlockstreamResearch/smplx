use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/simple_storage.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_simple_storage::SimpleStorageWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_simple_storage::SimpleStorageWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_simple_storage::SimpleStorageArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_simple_storage::SimpleStorageArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}
