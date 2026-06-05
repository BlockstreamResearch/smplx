use simplex::include_simf;
use simplex::program::{WitnessTrait, ArgumentsTrait};

include_simf!("../../../../crates/simplex/tests/ui_simfs/dual_currency_deposit.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness::default();

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_dual_currency_deposit::DualCurrencyDepositArguments::default();

    let arguments_values = original_arguments.build_arguments();
    let recovered_arguments =
        derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&arguments_values)?;
    assert_eq!(original_arguments, recovered_arguments);

    Ok(())
}
