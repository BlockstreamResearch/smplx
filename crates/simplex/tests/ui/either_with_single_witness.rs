use simplex::include_simf;
use simplex::program::{ArgumentsTrait, RandomArguments, RandomWitness, WitnessTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/either_with_single_witness.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_default();
    let _ = test_e2e_random_behaviour();

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    let original_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness =
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::from_witness(
            &witness_values,
        )?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments =
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments =
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::from_arguments(
            &arguments_values,
        )?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default(),
        derived_either_with_single_witness::EitherWithSingleWitnessWitness::default()
    );
    assert_eq!(
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default(),
        derived_either_with_single_witness::EitherWithSingleWitnessArguments::default()
    );
    Ok(())
}

fn test_e2e_random_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::generate_witness_raw(
                &mut rng,
            );

        let witness_values = original_witness.build_witness();
        let recovered_witness =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::from_witness(
                &witness_values,
            )?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_either_with_single_witness::EitherWithSingleWitnessWitness::generate_witness(
                &mut rng,
            );
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::generate_arguments_raw(
                &mut rng,
            );

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::from_arguments(
                &arguments_values,
            )?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values =
            derived_either_with_single_witness::EitherWithSingleWitnessArguments::generate_arguments(
                &mut rng,
            );
        assert_eq!(arguments_values, rand_raw_witness_values);
    }
    Ok(())
}
