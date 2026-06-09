use simplex::include_simf;
use simplex::program::{ArgumentsTrait, RandomArguments, RandomWitness, WitnessTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/single_bit.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_default();
    let _ = test_e2e_random_behaviour();

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    for (bit, flag) in [(1, 1), (1, 0), (0, 1), (0, 0)] {
        let original_witness = derived_single_bit::SingleBitWitness { bit };

        let witness_values = original_witness.build_witness();
        let recovered_witness =
            derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        let original_arguments = derived_single_bit::SingleBitArguments { flag };

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments =
            derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);
    }

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_single_bit::SingleBitWitness::default(),
        derived_single_bit::SingleBitWitness::default()
    );
    assert_eq!(
        derived_single_bit::SingleBitArguments::default(),
        derived_single_bit::SingleBitArguments::default()
    );
    Ok(())
}

fn test_e2e_random_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness = derived_single_bit::SingleBitWitness::generate_witness_raw(&mut rng);

        let witness_values = original_witness.build_witness();
        let recovered_witness =
            derived_single_bit::SingleBitWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_single_bit::SingleBitWitness::generate_witness(&mut rng);
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments =
            derived_single_bit::SingleBitArguments::generate_arguments_raw(&mut rng);

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments =
            derived_single_bit::SingleBitArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_single_bit::SingleBitArguments::generate_arguments(&mut rng);
        assert_eq!(arguments_values, rand_raw_witness_values);
    }
    Ok(())
}
