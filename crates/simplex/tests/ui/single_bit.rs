use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait, RandomArguments, RandomWitness};

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

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
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

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
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

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
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

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
    assert_eq!(
        original_arguments, recovered_arguments,
        "Testing values with (FLAG = 0, BIT = 0) (Arguments)"
    );

    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness = derived_single_bit::SingleBitWitness::generate_witness_raw(&mut rng);

        let witness_values = original_witness.build_witness();
        let recovered_witness = derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_single_bit::SingleBitWitness::generate_witness(&mut rng);
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments = derived_single_bit::SingleBitArguments::generate_arguments_raw(&mut rng);

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments = derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_single_bit::SingleBitArguments::generate_arguments(&mut rng);
        assert_eq!(arguments_values, rand_raw_witness_values);
    }

    Ok(())
}
