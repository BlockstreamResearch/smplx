use simplex::include_simf;
use simplex::program::{ArgumentsTrait, RandomArguments, RandomWitness, WitnessTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/list_check.simf");

fn main() -> Result<(), String> {
    let _ = test_e2e_behaviour()?;
    let _ = test_build_panic()?;
    let _ = test_default();
    let _ = test_e2e_random_behaviour();

    Ok(())
}

fn test_e2e_behaviour() -> Result<(), String> {
    let original_witness = derived_list_check::ListCheckWitness {
        draft: vec![],
        path: simplex::either::Left(vec![
            simplex::either::Either::Right((5, [0; 64], true)),
            simplex::either::Either::Left((5, 5, 5, 5)),
            simplex::either::Either::Left((5, 5, 5, 5)),
        ]),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_list_check::ListCheckWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_list_check::ListCheckArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments = derived_list_check::ListCheckArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}

fn test_build_panic() -> Result<(), String> {
    // Build Witness, which would panic on building
    let original_witness = derived_list_check::ListCheckWitness {
        draft: vec![],
        path: simplex::either::Left(vec![
            simplex::either::Either::Right((5, [0; 64], true)),
            simplex::either::Either::Left((5, 5, 5, 5)),
            simplex::either::Either::Left((5, 5, 5, 5)),
            simplex::either::Either::Right((5, [0; 64], true)),
        ]),
    };

    // Register panic hook to reduce warnings
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let result = std::panic::catch_unwind(|| original_witness.build_witness());
    std::panic::set_hook(default_hook);

    assert!(
        result.is_err(),
        "Expected build_witness to panic, as we have Vec size equal to list size, but it succeeded."
    );

    Ok(())
}

fn test_default() -> Result<(), String> {
    assert_eq!(
        derived_list_check::ListCheckWitness::default(),
        derived_list_check::ListCheckWitness::default()
    );
    assert_eq!(
        derived_list_check::ListCheckArguments::default(),
        derived_list_check::ListCheckArguments::default()
    );

    Ok(())
}

fn test_e2e_random_behaviour() -> Result<(), String> {
    for seed in 0..32 {
        use simplex::rand::{rngs::StdRng, SeedableRng};

        let mut rng = StdRng::seed_from_u64(seed);

        let original_witness = derived_list_check::ListCheckWitness::generate_witness_raw(&mut rng);

        let witness_values = original_witness.build_witness();
        let recovered_witness = derived_list_check::ListCheckWitness::from_witness(&witness_values)?;
        assert_eq!(original_witness, recovered_witness);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_list_check::ListCheckWitness::generate_witness(&mut rng);
        assert_eq!(witness_values, rand_raw_witness_values);

        rng = StdRng::seed_from_u64(seed);
        let original_arguments = derived_list_check::ListCheckArguments::generate_arguments_raw(&mut rng);

        let arguments_values = original_arguments.build_arguments();
        let recovered_arguments = derived_list_check::ListCheckArguments::from_arguments(&arguments_values)?;
        assert_eq!(original_arguments, recovered_arguments);

        rng = StdRng::seed_from_u64(seed);
        let rand_raw_witness_values = derived_list_check::ListCheckArguments::generate_arguments(&mut rng);
        assert_eq!(arguments_values, rand_raw_witness_values);
    }

    Ok(())
}
