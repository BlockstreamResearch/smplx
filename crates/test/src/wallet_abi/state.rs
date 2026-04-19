use std::sync::Arc;

use lwk_wollet::bitcoin::PublicKey;
use lwk_wollet::bitcoin::bip32::KeySource;
use lwk_wollet::elements::confidential::{Asset as ConfidentialAsset, Nonce, Value};
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::elements::{Address, EcdsaSighashType, OutPoint, Transaction, TxOut, TxOutSecrets, Txid};
use lwk_wollet::secp256k1::{All, Keypair, Secp256k1, XOnlyPublicKey};
use lwk_wollet::{ElementsNetwork, ExternalUtxo};
use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use smplx_sdk::provider::{ElementsRpc, ProviderInfo, ProviderTrait, SimplicityNetwork};
use smplx_sdk::signer::{Signer as SdkSigner, SignerTrait};
use smplx_sdk::transaction::UTXO;

use super::adapters::WalletAbiAdapterError;
use super::bootstrap::to_elements_network;
use super::types::RuntimeFundingAsset;
use crate::context::{TestContext, build_provider};
use crate::error::TestError;

const P2WPKH_MAX_WEIGHT_TO_SATISFY: usize = 128;

pub(crate) struct WalletAbiSharedState {
    pub(crate) sdk_network: SimplicityNetwork,
    pub(crate) network: ElementsNetwork,
    pub(crate) mnemonic: String,
    pub(crate) provider_info: ProviderInfo,
}

impl WalletAbiSharedState {
    pub(crate) fn new(context: &TestContext) -> Result<Self, TestError> {
        let sdk_network = *context.get_network();

        Ok(Self {
            sdk_network,
            network: to_elements_network(&sdk_network)?,
            mnemonic: context.mnemonic().to_string(),
            provider_info: context.provider_info().clone(),
        })
    }

    fn create_provider(&self) -> Box<dyn ProviderTrait> {
        build_provider(&self.provider_info, self.sdk_network)
    }

    fn create_sdk_signer(&self) -> SdkSigner {
        SdkSigner::new(self.mnemonic.as_str(), self.create_provider())
    }

    fn create_rpc(&self) -> Result<ElementsRpc, WalletAbiAdapterError> {
        let Some(url) = self.provider_info.elements_url.clone() else {
            return Err(WalletAbiAdapterError::MissingRpc);
        };
        let Some(auth) = self.provider_info.auth.clone() else {
            return Err(WalletAbiAdapterError::MissingRpc);
        };

        Ok(ElementsRpc::new(url, auth)?)
    }

    fn secp() -> Secp256k1<All> {
        Secp256k1::new()
    }

    fn owns_outpoint(&self, out_point: &OutPoint) -> Result<bool, WalletAbiAdapterError> {
        Ok(self.wallet_snapshot()?.iter().any(|utxo| &utxo.outpoint == out_point))
    }

    pub(crate) fn current_height(&self) -> Result<u32, WalletAbiAdapterError> {
        self.provider_tip_height()
    }

    pub(crate) fn provider_tip_height(&self) -> Result<u32, WalletAbiAdapterError> {
        Ok(self.create_provider().fetch_tip_height()?)
    }

    pub(crate) fn receive_address(&self) -> Result<Address, WalletAbiAdapterError> {
        Ok(self.create_sdk_signer().get_confidential_address())
    }

    pub(crate) fn generate_blocks(&self, blocks: u32) -> Result<(), WalletAbiAdapterError> {
        self.create_rpc()?.generate_blocks(blocks)?;
        Ok(())
    }

    pub(crate) fn broadcast(&self, tx: &Transaction) -> Result<Txid, WalletAbiAdapterError> {
        Ok(self.create_provider().broadcast_transaction(tx)?)
    }

    pub(crate) fn fetch_transaction(&self, txid: &Txid) -> Result<Transaction, WalletAbiAdapterError> {
        Ok(self.create_provider().fetch_transaction(txid)?)
    }

    pub(crate) fn fetch_tx_out(&self, outpoint: OutPoint) -> Result<TxOut, WalletAbiAdapterError> {
        let tx = self.fetch_transaction(&outpoint.txid)?;
        tx.output
            .get(outpoint.vout as usize)
            .cloned()
            .ok_or(WalletAbiAdapterError::MissingTxOutput {
                txid: outpoint.txid,
                vout: outpoint.vout,
            })
    }

    pub(crate) fn wallet_snapshot(&self) -> Result<Vec<ExternalUtxo>, WalletAbiAdapterError> {
        Ok(self
            .create_sdk_signer()
            .get_utxos()?
            .into_iter()
            .map(wallet_abi_external_utxo)
            .collect())
    }

    pub(crate) fn open_wallet_request_session(
        &self,
    ) -> Result<lwk_simplicity::wallet_abi::WalletRequestSession, WalletAbiAdapterError> {
        Ok(lwk_simplicity::wallet_abi::WalletRequestSession {
            session_id: lwk_simplicity::wallet_abi::generate_request_id().to_string(),
            network: self.network,
            spendable_utxos: Arc::from(self.wallet_snapshot()?),
        })
    }

    pub(crate) fn wallet_bip32_pair(
        &self,
        out_point: &OutPoint,
    ) -> Result<Option<(PublicKey, KeySource)>, WalletAbiAdapterError> {
        if !self.owns_outpoint(out_point)? {
            return Ok(None);
        }

        let signer = self.create_sdk_signer();

        Ok(Some((
            signer.get_ecdsa_public_key(),
            (signer.fingerprint()?, signer.get_derivation_path()?),
        )))
    }

    pub(crate) fn signer_xonly(&self) -> Result<XOnlyPublicKey, WalletAbiAdapterError> {
        Ok(self.signing_keypair()?.1)
    }

    pub(crate) fn signing_keypair(&self) -> Result<(Keypair, XOnlyPublicKey), WalletAbiAdapterError> {
        let private_key = self.create_sdk_signer().get_private_key();
        let keypair = Keypair::from_secret_key(&Self::secp(), &private_key.inner);
        let xonly = keypair.x_only_public_key().0;
        Ok((keypair, xonly))
    }

    pub(crate) fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), WalletAbiAdapterError> {
        let signer = self.create_sdk_signer();
        let signing_pubkey = signer.get_ecdsa_public_key();
        let mut partial_sigs = Vec::with_capacity(pst.inputs().len());

        for (index, input) in pst.inputs().iter().enumerate() {
            if !input.bip32_derivation.contains_key(&signing_pubkey) {
                partial_sigs.push(None);
                continue;
            }

            let (_, signature) = signer.sign_input(pst, index)?;
            let hash_ty = input
                .sighash_type
                .map(|value| value.ecdsa_hash_ty().unwrap_or(EcdsaSighashType::All))
                .unwrap_or(EcdsaSighashType::All);
            let mut raw_sig = signature.serialize_der().to_vec();
            raw_sig.push(hash_ty as u8);
            partial_sigs.push(Some(raw_sig));
        }

        for (input, raw_sig) in pst.inputs_mut().iter_mut().zip(partial_sigs) {
            if let Some(raw_sig) = raw_sig {
                input.partial_sigs.insert(signing_pubkey, raw_sig);
            }
        }

        Ok(())
    }

    pub(crate) fn unblind_txout(&self, tx_out: &TxOut) -> Result<TxOutSecrets, WalletAbiAdapterError> {
        match (tx_out.asset, tx_out.value, tx_out.nonce) {
            (ConfidentialAsset::Explicit(_), Value::Explicit(_), _) => Ok(explicit_txout_secrets(tx_out)),
            (ConfidentialAsset::Confidential(_), Value::Confidential(_), Nonce::Confidential(_)) => {
                let blinding_key = self.create_sdk_signer().get_blinding_private_key();
                Ok(tx_out.unblind(&Self::secp(), blinding_key.inner)?)
            }
            _ => Err(WalletAbiAdapterError::Invariant(
                "received unconfidential or null asset/value/nonce".to_string(),
            )),
        }
    }

    pub(crate) fn output_template(
        &self,
        _request: &lwk_simplicity::wallet_abi::WalletOutputRequest,
    ) -> Result<lwk_simplicity::wallet_abi::WalletOutputTemplate, WalletAbiAdapterError> {
        // The harness intentionally models a single-address wallet.
        let address = self.receive_address()?;

        Ok(lwk_simplicity::wallet_abi::WalletOutputTemplate {
            script_pubkey: address.script_pubkey(),
            blinding_pubkey: address.blinding_pubkey,
        })
    }

    pub(crate) fn fund_address(
        &self,
        address: &Address,
        asset: RuntimeFundingAsset,
        amount_sat: u64,
    ) -> Result<UTXO, TestError> {
        let rpc = self.create_rpc()?;
        let (funded_asset, txid) = match asset {
            RuntimeFundingAsset::Lbtc => (
                self.network.policy_asset(),
                rpc.send_to_address(address, amount_sat, None)?,
            ),
            RuntimeFundingAsset::IssuedAsset => {
                let issued = rpc.issue_asset(amount_sat)?;
                let txid = rpc.send_to_address(address, amount_sat, Some(issued))?;
                (issued, txid)
            }
        };
        let tx = rpc.get_raw_transaction(&txid)?;
        Ok(self.find_output(
            &tx,
            |tx_out| {
                if tx_out.script_pubkey != address.script_pubkey() {
                    return Ok(None);
                }

                let secrets = self.unblind_txout(tx_out)?;
                Ok((secrets.asset == funded_asset && secrets.value == amount_sat).then_some(Some(secrets)))
            },
            "funded output",
        )?)
    }

    pub(crate) fn find_matching_output(
        &self,
        tx: &Transaction,
        predicate: impl Fn(&TxOut) -> bool,
    ) -> Result<UTXO, WalletAbiAdapterError> {
        self.find_output(
            tx,
            |tx_out| {
                if predicate(tx_out) {
                    Ok(Some(self.unblind_txout(tx_out).ok()))
                } else {
                    Ok(None)
                }
            },
            "matching output",
        )
    }

    fn find_output(
        &self,
        tx: &Transaction,
        mut select: impl FnMut(&TxOut) -> Result<Option<Option<TxOutSecrets>>, WalletAbiAdapterError>,
        description: &str,
    ) -> Result<UTXO, WalletAbiAdapterError> {
        let mut matches = Vec::new();

        for (vout, tx_out) in tx.output.iter().enumerate() {
            if let Some(secrets) = select(tx_out)? {
                matches.push(UTXO {
                    outpoint: OutPoint::new(tx.txid(), vout as u32),
                    txout: tx_out.clone(),
                    secrets,
                });
            }
        }

        if matches.len() != 1 {
            return Err(WalletAbiAdapterError::Invariant(format!(
                "expected exactly one {description}, found {}",
                matches.len()
            )));
        }

        Ok(matches.remove(0))
    }
}

fn wallet_abi_external_utxo(utxo: UTXO) -> ExternalUtxo {
    let unblinded = utxo.secrets.unwrap_or_else(|| explicit_utxo_secrets(&utxo));

    ExternalUtxo {
        outpoint: utxo.outpoint,
        txout: utxo.txout,
        tx: None,
        unblinded,
        max_weight_to_satisfy: P2WPKH_MAX_WEIGHT_TO_SATISFY,
    }
}

fn explicit_utxo_secrets(utxo: &UTXO) -> TxOutSecrets {
    TxOutSecrets::new(
        utxo.asset_id(),
        AssetBlindingFactor::zero(),
        utxo.amount(),
        ValueBlindingFactor::zero(),
    )
}

fn explicit_txout_secrets(tx_out: &TxOut) -> TxOutSecrets {
    TxOutSecrets::new(
        tx_out.asset.explicit().expect("explicit asset checked by caller"),
        AssetBlindingFactor::zero(),
        tx_out.value.explicit().expect("explicit value checked by caller"),
        ValueBlindingFactor::zero(),
    )
}
