use std::collections::HashMap;

use simplex::lwk_simplicity::wallet_abi::{AmountFilter, AssetFilter, UTXOSource, WalletSourceFilter};
use simplex::wallet_abi::{
    ElementsSequence, FinalizerSpec, InternalKeySource, RuntimeFundingAsset, SimfArguments, SimfWitness,
    TxEvaluateRequest, WalletAbiHarness, serialize_arguments, serialize_witness,
};

#[simplex::test]
fn wallet_abi_smoke(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let lbtc_funding = harness.fund_signer_lbtc(200_000)?;
    let issued_funding = harness.fund_signer_asset(5_000)?;

    let wallet_utxos = harness.wallet_utxos()?;
    assert!(wallet_utxos.iter().any(|utxo| {
        utxo.unblinded.asset == lbtc_funding.asset_id() && utxo.unblinded.value == lbtc_funding.amount()
    }));
    assert!(wallet_utxos.iter().any(|utxo| {
        utxo.unblinded.asset == issued_funding.asset_id() && utxo.unblinded.value == issued_funding.amount()
    }));

    let recipient_script = harness.signer_script();
    let policy_asset = harness.network().policy_asset();
    let request = harness
        .tx()
        .raw_wallet_input(
            "wallet-input",
            UTXOSource::Wallet {
                filter: WalletSourceFilter {
                    asset: AssetFilter::Exact { asset_id: policy_asset },
                    amount: AmountFilter::Min { amount_sat: 50_000 },
                    lock: Default::default(),
                },
            },
            ElementsSequence::ENABLE_LOCKTIME_NO_RBF,
        )
        .explicit_output("recipient", recipient_script.clone(), policy_asset, 50_000)
        .build_create()?;
    let request_id = request.request_id.to_string();

    let preview = harness.evaluate_request(TxEvaluateRequest::from_parts(
        request_id.as_str(),
        request.network,
        request.params.clone(),
    )?)?;
    assert!(preview.error.is_none());
    assert!(preview.preview.is_some());

    let tx = harness.process_request(request)?;
    let recipient_output = harness.find_output(&tx, |tx_out| {
        tx_out.script_pubkey == recipient_script
            && tx_out.asset.explicit() == Some(policy_asset)
            && tx_out.value.explicit() == Some(50_000)
    })?;

    assert_eq!(recipient_output.amount(), 50_000);
    assert_eq!(recipient_output.asset_id(), policy_asset);

    Ok(())
}

#[simplex::test]
fn wallet_abi_current_height_matches_provider_tip(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    assert_eq!(harness.current_height()?, harness.provider_tip_height()?);

    Ok(())
}

#[simplex::test]
fn wallet_abi_mine_to_height_reaches_target(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let current_height = harness.current_height()?;
    let target_height = current_height + 2;

    harness.mine_to_height(target_height)?;
    assert!(harness.current_height()? >= target_height);

    harness.mine_to_height(target_height)?;
    assert!(harness.current_height()? >= target_height);

    Ok(())
}

#[simplex::test]
fn wallet_abi_fund_and_sync_returns_synced_utxo(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let funding = harness.fund_and_sync(harness.signer_address(), RuntimeFundingAsset::Lbtc, 200_000, 1)?;
    let wallet_utxos = harness.wallet_utxos()?;

    assert!(wallet_utxos.iter().any(|utxo| {
        utxo.outpoint == funding.outpoint
            && utxo.unblinded.asset == funding.asset_id()
            && utxo.unblinded.value == funding.amount()
    }));

    Ok(())
}

#[simplex::test]
fn wallet_abi_fund_signer_lbtc_returns_policy_asset(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let funding = harness.fund_signer_lbtc(200_000)?;

    assert_eq!(funding.asset_id(), harness.network().policy_asset());
    assert_eq!(funding.amount(), 200_000);

    Ok(())
}

#[simplex::test]
fn wallet_abi_fund_signer_asset_returns_issued_asset(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let funding = harness.fund_signer_asset(5_000)?;

    assert_ne!(funding.asset_id(), harness.network().policy_asset());
    assert_eq!(funding.amount(), 5_000);

    Ok(())
}

#[simplex::test]
fn simf_finalizer_uses_bip0341(context: simplex::TestContext) -> anyhow::Result<()> {
    let harness = WalletAbiHarness::from_test_context(context)?;

    let arguments = SimfArguments::new(simplex::simplicityhl::Arguments::from(HashMap::new()));
    let witness = SimfWitness::new(simplex::simplicityhl::WitnessValues::from(HashMap::new()));
    let expected_arguments = serialize_arguments(&arguments)?;
    let expected_witness = serialize_witness(&witness)?;

    let finalizer = harness.simf_finalizer("unit.simf", &arguments, &witness)?;

    match finalizer {
        FinalizerSpec::Simf {
            source_simf,
            internal_key,
            arguments,
            witness,
        } => {
            assert_eq!(source_simf, "unit.simf");
            assert_eq!(internal_key, InternalKeySource::Bip0341);
            assert_eq!(arguments, expected_arguments);
            assert_eq!(witness, expected_witness);
        }
        FinalizerSpec::Wallet => panic!("expected simf finalizer"),
    }

    Ok(())
}
