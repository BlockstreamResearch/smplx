use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/bytes32_tr_storage.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_bytes32_tr_storage::Bytes32TrStorageWitness {
        state: Default::default(),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_bytes32_tr_storage::Bytes32TrStorageWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_bytes32_tr_storage::Bytes32TrStorageArguments {};

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_bytes32_tr_storage::Bytes32TrStorageArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}
