use simplex::include_simf;
use simplex::program::{ArgumentsTrait, WitnessTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/array_tr_storage.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_array_tr_storage::ArrayTrStorageWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_array_tr_storage::ArrayTrStorageWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_array_tr_storage::ArrayTrStorageArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_array_tr_storage::ArrayTrStorageArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}
