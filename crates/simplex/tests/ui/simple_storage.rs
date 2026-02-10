use simplex_macros::*;

include_simf!("../../../../crates/simplex/tests/ui/simple_storage.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_simple_storage::SimpleStorageWitness {
        new_value: 0,
        user_signature: [1; 64],
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_simple_storage::SimpleStorageWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_simple_storage::SimpleStorageArguments { user: Default::default(), slot_id: Default::default() };

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_simple_storage::SimpleStorageArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    let _template = derived_simple_storage::get_template_program();
    let _compiled = derived_simple_storage::get_compiled_program(&original_arguments);

    Ok(())
}