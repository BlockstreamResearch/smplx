use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait, RandomArguments, RandomWitness};

include_simf!("../../../../crates/simplex/tests/ui_simfs/options.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_options::OptionsWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_options::OptionsWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_options::OptionsArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_options::OptionsArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness = derived_options::OptionsWitness::generate_witness_raw(&mut rng);

        let witness_values = original_witness.build_witness();
        let recovered_witness = derived_options::OptionsWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_options::OptionsWitness::generate_witness(&mut rng);
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments = derived_options::OptionsArguments::generate_arguments_raw(&mut rng);

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments = derived_options::OptionsArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_options::OptionsArguments::generate_arguments(&mut rng);
        assert_eq!(arguments_values, rand_raw_witness_values);
    };

    Ok(())
}
