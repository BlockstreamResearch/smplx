use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/list_check.simf");

fn main() -> Result<(), String> {
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

    let original_arguments = derived_list_check::ListCheckArguments {};

    let witness_values = original_arguments.build_arguments();
    let recovered_witness = derived_list_check::ListCheckArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

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
    let result = std::panic::catch_unwind(|| {
        original_witness.build_witness()
    });
    std::panic::set_hook(default_hook);

    assert!(result.is_err(), "Expected build_witness to panic, as we have Vec size equal to list size, but it succeeded.");

    Ok(())
}

