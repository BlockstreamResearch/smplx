use std::future::Future;
use std::path::Path;
use std::sync::Arc;

use super::WalletAbiRequestBuilder;
use super::adapters::{SimplexWalletAbiProvider, WalletAbiRuntimeAdapters};
use super::types::RuntimeFundingAsset;
use crate::config::TestConfig;
use crate::context::TestContext;
use crate::error::TestError;
use crate::wallet_abi::state::WalletAbiSharedState;
use hex::FromHex;
use lwk_simplicity::error::WalletAbiError;
use lwk_simplicity::wallet_abi::{
    FinalizerSpec, InternalKeySource, SimfArguments, SimfWitness, TransactionInfo, TxCreateRequest, TxCreateResponse,
    TxEvaluateRequest, TxEvaluateResponse, serialize_arguments, serialize_witness,
};
use lwk_wollet::ExternalUtxo;
use lwk_wollet::elements::encode::deserialize;
use lwk_wollet::elements::{Address, Script, Transaction, TxOut};
use smplx_sdk::transaction::UTXO;

pub struct WalletAbiHarness {
    _context: TestContext,
    shared: Arc<WalletAbiSharedState>,
    runtime_adapters: WalletAbiRuntimeAdapters,
    signer_address: Address,
}

impl WalletAbiHarness {
    pub fn from_config_path(config_path: impl AsRef<Path>) -> Result<Self, TestError> {
        Self::from_test_context(TestContext::new(config_path.as_ref().to_path_buf())?)
    }

    pub fn from_test_config(config: &TestConfig) -> Result<Self, TestError> {
        Self::from_test_context(TestContext::from_test_config(config.clone())?)
    }

    pub fn from_test_context(context: TestContext) -> Result<Self, TestError> {
        let shared = Arc::new(WalletAbiSharedState::new(&context)?);
        let runtime_adapters = WalletAbiRuntimeAdapters::new(Arc::clone(&shared));
        let signer_address = shared.receive_address()?;

        Ok(Self {
            _context: context,
            runtime_adapters,
            shared,
            signer_address,
        })
    }

    pub fn signer_address(&self) -> &Address {
        &self.signer_address
    }

    pub fn signer_script(&self) -> Script {
        self.signer_address.script_pubkey()
    }

    pub fn network(&self) -> lwk_wollet::ElementsNetwork {
        self.shared.network
    }

    pub fn context(&self) -> &TestContext {
        &self._context
    }

    pub fn tx(&self) -> WalletAbiRequestBuilder {
        WalletAbiRequestBuilder::new(self.network())
    }

    pub fn wallet_utxos(&self) -> Result<Vec<ExternalUtxo>, TestError> {
        Ok(self.shared.wallet_snapshot()?)
    }

    pub fn current_height(&self) -> Result<u32, TestError> {
        Ok(self.shared.current_height()?)
    }

    pub fn provider_tip_height(&self) -> Result<u32, TestError> {
        Ok(self.shared.provider_tip_height()?)
    }

    pub fn mine_blocks(&self, blocks: u32) -> Result<(), TestError> {
        self.shared.generate_blocks(blocks)?;
        Ok(())
    }

    pub fn mine_to_height(&self, target_height: u32) -> Result<(), TestError> {
        let current_height = self.current_height()?;
        if target_height <= current_height {
            return Ok(());
        }

        self.mine_blocks(target_height - current_height)?;
        for _ in 0..20 {
            let synced_height = self.current_height()?;
            if synced_height >= target_height {
                return Ok(());
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Err(TestError::WalletAbiInvariant(format!(
            "failed to sync wallet tip to height {target_height}",
        )))
    }

    pub fn mine_and_sync(&self, blocks: u32) -> Result<(), TestError> {
        self.mine_blocks(blocks)
    }

    pub fn fund_address(
        &self,
        address: &Address,
        asset: RuntimeFundingAsset,
        amount_sat: u64,
    ) -> Result<UTXO, TestError> {
        self.shared.fund_address(address, asset, amount_sat)
    }

    pub fn fund_and_sync(
        &self,
        address: &Address,
        asset: RuntimeFundingAsset,
        amount_sat: u64,
        blocks: u32,
    ) -> Result<UTXO, TestError> {
        let utxo = self.fund_address(address, asset, amount_sat)?;
        self.mine_and_sync(blocks)?;
        self.wait_for_wallet_utxo(utxo.outpoint)?;
        Ok(utxo)
    }

    pub fn fund_signer_lbtc(&self, amount_sat: u64) -> Result<UTXO, TestError> {
        self.fund_and_sync(self.signer_address(), RuntimeFundingAsset::Lbtc, amount_sat, 1)
    }

    pub fn fund_signer_asset(&self, amount_sat: u64) -> Result<UTXO, TestError> {
        self.fund_and_sync(self.signer_address(), RuntimeFundingAsset::IssuedAsset, amount_sat, 1)
    }

    pub fn simf_finalizer(
        &self,
        source_simf: impl Into<String>,
        arguments: &SimfArguments,
        witness: &SimfWitness,
    ) -> Result<FinalizerSpec, TestError> {
        Ok(FinalizerSpec::Simf {
            source_simf: source_simf.into(),
            internal_key: InternalKeySource::Bip0341,
            arguments: serialize_arguments(arguments)?,
            witness: serialize_witness(witness)?,
        })
    }

    pub fn evaluate_request(&self, request: TxEvaluateRequest) -> Result<TxEvaluateResponse, TestError> {
        let provider = self.provider();
        self.run_async(move || async move { provider.evaluate_request(request).await })
    }

    pub fn process_request_response(&self, request: TxCreateRequest) -> Result<TxCreateResponse, TestError> {
        let should_mine = request.broadcast;
        let provider = self.provider();
        let response = self.run_async(move || async move { provider.process_request(request).await })?;
        if should_mine {
            self.mine_and_sync(1)?;
        }
        Ok(response)
    }

    pub fn process_request(&self, request: TxCreateRequest) -> Result<Transaction, TestError> {
        let response = self.process_request_response(request)?;
        decode_transaction(
            &response
                .transaction
                .ok_or_else(|| TestError::WalletAbiInvariant("missing transaction info".to_string()))?,
        )
    }

    pub fn find_output(&self, tx: &Transaction, predicate: impl Fn(&TxOut) -> bool) -> Result<UTXO, TestError> {
        Ok(self.shared.find_matching_output(tx, predicate)?)
    }

    fn provider(&self) -> SimplexWalletAbiProvider {
        self.runtime_adapters.provider()
    }

    fn wait_for_wallet_utxo(&self, outpoint: lwk_wollet::elements::OutPoint) -> Result<(), TestError> {
        for _ in 0..20 {
            let wallet_utxos = self.shared.wallet_snapshot()?;
            if wallet_utxos.iter().any(|utxo| utxo.outpoint == outpoint) {
                return Ok(());
            }

            std::thread::sleep(std::time::Duration::from_millis(100));
        }

        Err(TestError::WalletAbiInvariant(format!(
            "failed to observe wallet utxo {outpoint} after funding",
        )))
    }

    fn run_async<T, Fut, F>(&self, make_future: F) -> Result<T, TestError>
    where
        Fut: Future<Output = Result<T, WalletAbiError>>,
        F: FnOnce() -> Fut,
    {
        Ok(futures::executor::block_on(make_future())?)
    }
}

fn decode_transaction(info: &TransactionInfo) -> Result<Transaction, TestError> {
    let bytes =
        Vec::<u8>::from_hex(info.tx_hex.as_str()).map_err(|error| TestError::WalletAbiInvariant(error.to_string()))?;
    deserialize(&bytes).map_err(|error| TestError::WalletAbiInvariant(error.to_string()))
}
