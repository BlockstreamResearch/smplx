use simplex_macros::*;

include_simf!("../../../../crates/simplex/tests/ui/array_tr_storage.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_array_tr_storage::ArrayTrStorageWitness {
        changed_index: 0,
        state: Default::default(),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_array_tr_storage::ArrayTrStorageWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_array_tr_storage::ArrayTrStorageArguments {};

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_array_tr_storage::ArrayTrStorageArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}