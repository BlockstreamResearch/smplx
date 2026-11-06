use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer as SdkSigner;

use wasm_bindgen::prelude::*;

use crate::network::network_from_str;
use crate::signed_transaction::SignedTransaction;
use crate::transaction_builder::TransactionBuilder;

/// The wallet's signer that understands how to work with Simplicity.
#[wasm_bindgen]
pub struct Signer {
    signer: SdkSigner,
    network: SimplicityNetwork,
}

#[wasm_bindgen]
impl Signer {
    /// Creates a signer from an account mnemonic.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown.
    #[wasm_bindgen(constructor)]
    pub fn new(mnemonic: &str, network: &str) -> Result<Signer, JsError> {
        let network = network_from_str(network)?;

        Ok(Self {
            signer: SdkSigner::from_mnemonic(mnemonic, network),
            network,
        })
    }

    /// The WPKH address of the signer's own key.
    #[wasm_bindgen(js_name = address)]
    #[must_use]
    pub fn address(&self) -> String {
        let _ = &self.network;

        self.signer.get_address().to_string()
    }

    /// The confidential WPKH address of the signer's own key.
    #[wasm_bindgen(js_name = confidentialAddress)]
    #[must_use]
    pub fn confidential_address(&self) -> String {
        self.signer.get_confidential_address().to_string()
    }

    /// The x-only public key used for Schnorr and taproot, as lowercase hex.
    #[wasm_bindgen(js_name = schnorrPublicKey)]
    #[must_use]
    pub fn schnorr_public_key(&self) -> String {
        hex::encode(self.signer.get_schnorr_public_key().serialize())
    }

    /// The compressed public key used for native inputs, as lowercase hex.
    #[wasm_bindgen(js_name = ecdsaPublicKey)]
    #[must_use]
    pub fn ecdsa_public_key(&self) -> String {
        hex::encode(self.signer.get_ecdsa_public_key().to_bytes())
    }

    /// The scriptPubKey of the signer's own address, as lowercase hex.
    #[wasm_bindgen(js_name = scriptPubKeyHex)]
    #[must_use]
    pub fn script_pubkey_hex(&self) -> String {
        hex::encode(self.signer.get_address().script_pubkey().as_bytes())
    }

    /// The blinding public key, as lowercase hex.
    #[wasm_bindgen(js_name = blindingPublicKey)]
    #[must_use]
    pub fn blinding_public_key(&self) -> String {
        hex::encode(self.signer.get_blinding_public_key().to_bytes())
    }

    /// The fee `finalizeStrict` would pay for an assembled transaction at `fee_rate` sats/kvb, in
    /// satoshis.
    ///
    /// # Errors
    /// Returns an error if the transaction cannot be completed or a program rejects the spend.
    #[wasm_bindgen(js_name = estimateStrict)]
    pub fn estimate_strict(&self, builder: &TransactionBuilder, fee_rate: f32) -> Result<u64, JsError> {
        self.signer
            .estimate_strict(&builder.transaction, fee_rate)
            .map_err(|e| JsError::new(&format!("Could not estimate the transaction fee: {e}")))
    }

    /// Blinds, signs and finalizes an assembled transaction, adding a change output and a fee
    /// output at `fee_rate` sats/kvb.
    ///
    /// # Errors
    /// Returns an error if the transaction cannot be balanced, blinded, signed or finalized.
    #[wasm_bindgen(js_name = finalizeStrict)]
    pub fn finalize_strict(&self, builder: &TransactionBuilder, fee_rate: f32) -> Result<SignedTransaction, JsError> {
        let (transaction, fee_sats) = self
            .signer
            .finalize_strict(&builder.transaction, fee_rate)
            .map_err(|e| JsError::new(&format!("Could not finalize the transaction: {e}")))?;

        Ok(SignedTransaction::new(&transaction, fee_sats))
    }

    /// Blinds, signs and finalizes an assembled transaction with no change output, paying its
    /// whole policy-asset surplus as the fee.
    ///
    /// The surplus must cover the fee `estimateStrictFee` returns at the same `fee_rate`. The fee
    /// paid is the inputs minus the outputs, so it is known before signing.
    ///
    /// # Errors
    /// Returns an error if the surplus is below that fee, or if the transaction cannot be
    /// balanced, blinded, signed or finalized.
    #[wasm_bindgen(js_name = finalizeStrictFee)]
    pub fn finalize_strict_fee(
        &self,
        builder: &TransactionBuilder,
        fee_rate: f32,
    ) -> Result<SignedTransaction, JsError> {
        let (transaction, fee_sats) = self
            .signer
            .finalize_strict_fee(&builder.transaction, fee_rate)
            .map_err(|e| JsError::new(&format!("Could not finalize the transaction: {e}")))?;

        Ok(SignedTransaction::new(&transaction, fee_sats))
    }

    /// The fee `finalizeStrictFee` needs an assembled transaction to pay on `network` at
    /// `fee_rate` sats/kvb. Needs no key, so it can price a spend before the wallet is unlocked.
    ///
    /// Every program input runs with its signature checks passing, and every signature is
    /// measured at the largest size the signer makes. Returns the fee even if the transaction's
    /// surplus can't cover the fee.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown, an input's amount is unknown, an input is
    /// spent twice, a confidential input has no blinded output, or a program rejects the spend.
    #[wasm_bindgen(js_name = estimateStrictFee)]
    pub fn estimate_strict_fee(builder: &TransactionBuilder, network: &str, fee_rate: f32) -> Result<u64, JsError> {
        SdkSigner::estimate_strict_fee(&builder.transaction, &network_from_str(network)?, fee_rate)
            .map_err(|e| JsError::new(&format!("Could not estimate the transaction fee: {e}")))
    }
}
