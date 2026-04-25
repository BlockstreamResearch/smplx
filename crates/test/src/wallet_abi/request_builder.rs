use crate::error::TestError;
use lwk_simplicity::wallet_abi::schema::OutputSchema;
use lwk_simplicity::wallet_abi::{
    AmountFilter, AssetFilter, AssetVariant, BlinderVariant, FinalizerSpec, InputIssuance, InputIssuanceKind,
    InputSchema, InputUnblinding, LockFilter, LockVariant, RuntimeParams, TxCreateRequest, UTXOSource,
    WalletSourceFilter, generate_request_id,
};
use lwk_wollet::ElementsNetwork;
use lwk_wollet::elements::{AssetId, Script, Sequence};
use smplx_sdk::transaction::UTXO;

#[derive(Debug, Clone)]
pub struct WalletAbiRequestBuilder {
    network: ElementsNetwork,
    inputs: Vec<InputSchema>,
    outputs: Vec<OutputSchema>,
    lock_time: Option<lwk_wollet::elements::LockTime>,
}

impl WalletAbiRequestBuilder {
    pub(crate) fn new(network: ElementsNetwork) -> Self {
        Self {
            network,
            inputs: Vec::new(),
            outputs: Vec::new(),
            lock_time: None,
        }
    }

    pub fn build_create(self) -> Result<TxCreateRequest, TestError> {
        let request_id = generate_request_id().to_string();
        Ok(TxCreateRequest::from_parts(
            request_id.as_str(),
            self.network,
            RuntimeParams {
                inputs: self.inputs,
                outputs: self.outputs,
                fee_rate_sat_kvb: Some(100.0),
                lock_time: self.lock_time,
            },
            true,
        )?)
    }

    pub fn raw_wallet_input(mut self, id: impl Into<String>, utxo_source: UTXOSource, sequence: Sequence) -> Self {
        self.inputs.push(Self::input_schema(
            id,
            utxo_source,
            sequence,
            InputUnblinding::Wallet,
            FinalizerSpec::Wallet,
        ));
        self
    }

    pub fn raw_input_schema(mut self, schema: InputSchema) -> Self {
        self.inputs.push(schema);
        self
    }

    pub fn wallet_input_exact(mut self, id: impl Into<String>, asset_id: AssetId, amount_sat: u64) -> Self {
        self.inputs.push(Self::input_schema(
            id,
            UTXOSource::Wallet {
                filter: WalletSourceFilter {
                    asset: AssetFilter::Exact { asset_id },
                    amount: AmountFilter::Exact { amount_sat },
                    lock: LockFilter::None,
                },
            },
            Sequence::ENABLE_LOCKTIME_NO_RBF,
            InputUnblinding::Wallet,
            FinalizerSpec::Wallet,
        ));
        self
    }

    pub fn provided_input(
        mut self,
        id: impl Into<String>,
        utxo: &UTXO,
        unblinding: InputUnblinding,
        finalizer: FinalizerSpec,
    ) -> Self {
        self.inputs.push(Self::input_schema(
            id,
            UTXOSource::Provided {
                outpoint: utxo.outpoint,
            },
            Sequence::ENABLE_LOCKTIME_NO_RBF,
            unblinding,
            finalizer,
        ));
        self
    }

    pub fn new_issuance(
        mut self,
        input_id: &str,
        asset_amount_sat: u64,
        token_amount_sat: u64,
        entropy: [u8; 32],
    ) -> Result<Self, TestError> {
        let mut matches = self.inputs.iter_mut().filter(|input| input.id == input_id);
        let input = matches
            .next()
            .ok_or_else(|| TestError::WalletAbiInvariant(format!("missing input '{input_id}'")))?;
        if matches.next().is_some() {
            return Err(TestError::WalletAbiInvariant(format!(
                "expected exactly one input named '{input_id}'",
            )));
        }
        input.issuance = Some(InputIssuance {
            kind: InputIssuanceKind::New,
            asset_amount_sat,
            token_amount_sat,
            entropy,
        });
        Ok(self)
    }

    pub fn explicit_output(
        mut self,
        id: impl Into<String>,
        script: Script,
        asset_id: AssetId,
        amount_sat: u64,
    ) -> Self {
        self.outputs.push(Self::output_schema(
            id,
            amount_sat,
            LockVariant::Script { script },
            AssetVariant::AssetId { asset_id },
        ));
        self
    }

    pub fn new_issuance_asset_output(
        mut self,
        id: impl Into<String>,
        script: Script,
        input_index: u32,
        amount_sat: u64,
    ) -> Self {
        self.outputs.push(Self::output_schema(
            id,
            amount_sat,
            LockVariant::Script { script },
            AssetVariant::NewIssuanceAsset { input_index },
        ));
        self
    }

    pub fn finalizer_output(
        mut self,
        id: impl Into<String>,
        finalizer: FinalizerSpec,
        asset_id: AssetId,
        amount_sat: u64,
    ) -> Self {
        self.outputs.push(Self::output_schema(
            id,
            amount_sat,
            LockVariant::Finalizer {
                finalizer: Box::new(finalizer),
            },
            AssetVariant::AssetId { asset_id },
        ));
        self
    }

    pub fn raw_output(mut self, id: impl Into<String>, lock: LockVariant, asset_id: AssetId, amount_sat: u64) -> Self {
        self.outputs.push(Self::output_schema(
            id,
            amount_sat,
            lock,
            AssetVariant::AssetId { asset_id },
        ));
        self
    }

    pub fn raw_output_schema(mut self, schema: OutputSchema) -> Self {
        self.outputs.push(schema);
        self
    }

    pub fn lock_time_height(mut self, height: u32) -> Result<Self, TestError> {
        self.lock_time = Some(
            lwk_wollet::elements::LockTime::from_height(height)
                .map_err(|error| TestError::WalletAbiInvariant(error.to_string()))?,
        );
        Ok(self)
    }

    fn input_schema(
        id: impl Into<String>,
        utxo_source: UTXOSource,
        sequence: Sequence,
        unblinding: InputUnblinding,
        finalizer: FinalizerSpec,
    ) -> InputSchema {
        InputSchema {
            id: id.into(),
            utxo_source,
            unblinding,
            sequence,
            issuance: None,
            finalizer,
        }
    }

    fn output_schema(id: impl Into<String>, amount_sat: u64, lock: LockVariant, asset: AssetVariant) -> OutputSchema {
        OutputSchema {
            id: id.into(),
            amount_sat,
            lock,
            asset,
            blinder: BlinderVariant::Explicit,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet_abi::{AssetVariant, BlinderVariant, InternalKeySource, LockVariant, OutputSchema};
    use lwk_wollet::elements::{OutPoint, TxOut};

    #[test]
    fn build_create_uses_wallet_abi_defaults() {
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .build_create()
            .expect("request");

        assert_eq!(request.network, ElementsNetwork::Liquid);
        assert!(request.params.inputs.is_empty());
        assert!(request.params.outputs.is_empty());
        assert_eq!(request.params.fee_rate_sat_kvb, Some(100.0));
        assert_eq!(request.params.lock_time, None);
        assert!(request.broadcast);
        assert_eq!(request.abi_version, crate::wallet_abi::TX_CREATE_ABI_VERSION);
    }

    #[test]
    fn adds_wallet_input_exact() {
        let network = ElementsNetwork::Liquid;
        let request = WalletAbiRequestBuilder::new(network)
            .wallet_input_exact("wallet-input", network.policy_asset(), 50_000)
            .build_create()
            .expect("request");

        assert_eq!(request.params.inputs.len(), 1);
        let input = &request.params.inputs[0];
        assert_eq!(input.id, "wallet-input");
        assert_eq!(input.unblinding, InputUnblinding::Wallet);
        assert_eq!(input.sequence, Sequence::ENABLE_LOCKTIME_NO_RBF);
        assert_eq!(input.issuance, None);
        assert_eq!(input.finalizer, FinalizerSpec::Wallet);
        assert_eq!(
            input.utxo_source,
            UTXOSource::Wallet {
                filter: WalletSourceFilter {
                    asset: AssetFilter::Exact {
                        asset_id: network.policy_asset(),
                    },
                    amount: AmountFilter::Exact { amount_sat: 50_000 },
                    lock: LockFilter::None,
                },
            }
        );
    }

    #[test]
    fn adds_explicit_output() {
        let script = Script::new();
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .explicit_output(
                "recipient",
                script.clone(),
                ElementsNetwork::Liquid.policy_asset(),
                50_000,
            )
            .build_create()
            .expect("request");

        assert_eq!(
            request.params.outputs,
            vec![OutputSchema {
                id: "recipient".to_string(),
                amount_sat: 50_000,
                lock: LockVariant::Script { script },
                asset: AssetVariant::AssetId {
                    asset_id: ElementsNetwork::Liquid.policy_asset(),
                },
                blinder: BlinderVariant::Explicit,
            }]
        );
    }

    #[test]
    fn adds_provided_input() {
        let known_utxo = UTXO {
            outpoint: OutPoint::default(),
            txout: TxOut::default(),
            secrets: None,
        };
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .provided_input(
                "provided-input",
                &known_utxo,
                InputUnblinding::Explicit,
                FinalizerSpec::Wallet,
            )
            .build_create()
            .expect("request");

        assert_eq!(request.params.inputs.len(), 1);
        let input = &request.params.inputs[0];
        assert_eq!(input.id, "provided-input");
        assert_eq!(
            input.utxo_source,
            UTXOSource::Provided {
                outpoint: OutPoint::default(),
            }
        );
        assert_eq!(input.unblinding, InputUnblinding::Explicit);
        assert_eq!(input.sequence, Sequence::ENABLE_LOCKTIME_NO_RBF);
        assert_eq!(input.issuance, None);
        assert_eq!(input.finalizer, FinalizerSpec::Wallet);
    }

    #[test]
    fn sets_new_issuance_on_named_input() {
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .wallet_input_exact("issuance-input", ElementsNetwork::Liquid.policy_asset(), 100_000)
            .new_issuance("issuance-input", 50_000, 0, [7; 32])
            .expect("builder")
            .build_create()
            .expect("request");

        assert_eq!(
            request.params.inputs[0].issuance,
            Some(InputIssuance {
                kind: InputIssuanceKind::New,
                asset_amount_sat: 50_000,
                token_amount_sat: 0,
                entropy: [7; 32],
            })
        );
    }

    #[test]
    fn adds_new_issuance_asset_output() {
        let script = Script::new();
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .new_issuance_asset_output("issued", script.clone(), 2, 25_000)
            .build_create()
            .expect("request");

        assert_eq!(
            request.params.outputs,
            vec![OutputSchema {
                id: "issued".to_string(),
                amount_sat: 25_000,
                lock: LockVariant::Script { script },
                asset: AssetVariant::NewIssuanceAsset { input_index: 2 },
                blinder: BlinderVariant::Explicit,
            }]
        );
    }

    #[test]
    fn adds_finalizer_output() {
        let finalizer = FinalizerSpec::Simf {
            source_simf: "unit.simf".to_string(),
            internal_key: InternalKeySource::Bip0341,
            arguments: vec![1],
            witness: vec![2],
        };
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .finalizer_output(
                "locked",
                finalizer.clone(),
                ElementsNetwork::Liquid.policy_asset(),
                10_000,
            )
            .build_create()
            .expect("request");

        assert_eq!(
            request.params.outputs,
            vec![OutputSchema {
                id: "locked".to_string(),
                amount_sat: 10_000,
                lock: LockVariant::Finalizer {
                    finalizer: Box::new(finalizer),
                },
                asset: AssetVariant::AssetId {
                    asset_id: ElementsNetwork::Liquid.policy_asset(),
                },
                blinder: BlinderVariant::Explicit,
            }]
        );
    }

    #[test]
    fn sets_lock_time_from_height() {
        let request = WalletAbiRequestBuilder::new(ElementsNetwork::Liquid)
            .lock_time_height(42)
            .expect("builder")
            .build_create()
            .expect("request");

        assert_eq!(
            request.params.lock_time,
            Some(lwk_wollet::elements::LockTime::from_height(42).expect("height lock")),
        );
    }
}
