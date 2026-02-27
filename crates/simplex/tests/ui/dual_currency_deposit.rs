use simplex_macros::*;
use simplex_sdk::witness::WitnessTrait;
use simplex_sdk::arguments::ArgumentsTrait;

include_simf!("../../../../crates/simplex/tests/ui/dual_currency_deposit.simf");

fn main() -> Result<(), String> {
    let original_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness {
        merge_branch: simplex::either::Left(simplex::either::Right(())),
        token_branch: simplex::either::Left(()),
        path: simplex::either::Left(simplex::either::Left(simplex::either::Left((0, 1, 2, 3)))),
    };

    let witness_values = original_witness.build_witness();
    let recovered_witness = derived_dual_currency_deposit::DualCurrencyDepositWitness::from_witness(&witness_values)?;
    assert_eq!(original_witness, recovered_witness);

    let original_arguments = derived_dual_currency_deposit::DualCurrencyDepositArguments {
        grantor_per_settlement_asset: 0,
        settlement_asset_id: [1; 32],
        grantor_settlement_token_asset: [1; 32],
        strike_price: 0,
        incentive_basis_points: 0,
        grantor_collateral_token_asset: [1; 32],
        contract_expiry_time: 0,
        filler_per_settlement_asset: 0,
        filler_per_principal_collateral: 0,
        filler_token_asset: [1; 32],
        grantor_per_settlement_collateral: 0,
        grantor_settlement_per_deposited_asset: 0,
        fee_script_hash: [1; 32],
        taker_funding_end_time: 0,
        settlement_height: 0,
        collateral_asset_id: [1; 32],
        taker_funding_start_time: 0,
        filler_per_settlement_collateral: 0,
        oracle_pk: [1; 32],
        fee_basis_points: 0,
        grantor_collateral_per_deposited_collateral: 0,
        early_termination_end_time: 0,
    };

    let witness_values = original_arguments.build_arguments();
    let recovered_witness =
        derived_dual_currency_deposit::DualCurrencyDepositArguments::from_arguments(&witness_values)?;
    assert_eq!(original_arguments, recovered_witness);

    Ok(())
}