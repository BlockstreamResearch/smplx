use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/single_bit.simf");

fn main() -> Result<(), String> {
    // Testing values with (FLAG = 1, BIT = 1)
    let original_witness = derived_single_bit::SingleBitWitness { bit: 1 };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
    assert_eq!(
        original_witness, recovered_witness,
        "Testing values with (FLAG = 1, BIT = 1) (Witness)"
    );

    let original_arguments = derived_single_bit::SingleBitArguments { flag: 1 };

    let witness_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&witness_values)?;
    assert_eq!(
        original_arguments, recovered_arguments,
        "Testing values with (FLAG = 1, BIT = 1) (Arguments)"
    );

    // Testing values with (FLAG = 0, BIT = 1)
    let original_witness = derived_single_bit::SingleBitWitness { bit: 1 };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
    assert_eq!(
        original_witness, recovered_witness,
        "Testing values with (FLAG = 0, BIT = 1) (Witness)"
    );

    let original_arguments = derived_single_bit::SingleBitArguments { flag: 0 };

    let witness_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&witness_values)?;
    assert_eq!(
        original_arguments, recovered_arguments,
        "Testing values with (FLAG = 0, BIT = 1) (Arguments)"
    );

    // Testing values with (FLAG = 1, BIT = 0)
    let original_witness = derived_single_bit::SingleBitWitness { bit: 0 };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
    assert_eq!(
        original_witness, recovered_witness,
        "Testing values with (FLAG = 1, BIT = 0) (Witness)"
    );

    let original_arguments = derived_single_bit::SingleBitArguments { flag: 1 };

    let witness_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&witness_values)?;
    assert_eq!(
        original_arguments, recovered_arguments,
        "Testing values with (FLAG = 1, BIT = 0) (Arguments)"
    );

    // Testing values with (FLAG = 0, BIT = 0)
    let original_witness = derived_single_bit::SingleBitWitness { bit: 0 };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
    assert_eq!(
        original_witness, recovered_witness,
        "Testing values with (FLAG = 0, BIT = 0) (Witness)"
    );

    let original_arguments = derived_single_bit::SingleBitArguments { flag: 0 };

    let witness_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&witness_values)?;
    assert_eq!(
        original_arguments, recovered_arguments,
        "Testing values with (FLAG = 0, BIT = 0) (Arguments)"
    );

    Ok(())
}
