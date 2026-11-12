use elements_miniscript::bitcoin::PublicKey;

use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
use simplicityhl::elements::pset::Output;
use simplicityhl::elements::{AssetId, Script};

/// An output of a [`FinalTransaction`](super::FinalTransaction), with an explicit amount and asset
/// that the signer blinds if it has a blinding key.
///
/// A fee output is `PartialOutput::new(Script::new(), fee, network.policy_asset())`. The signer's
/// `finalize*` methods and their estimates add one.
#[derive(Debug, Clone)]
pub struct PartialOutput {
    /// The script the output pays to.
    pub script_pubkey: Script,
    /// The amount, in the asset's base units: satoshis for the policy asset.
    pub amount: u64,
    /// The asset.
    pub asset: AssetId,
    /// The blinding public key. With one, the signer blinds this output's amount and asset.
    pub blinding_key: Option<PublicKey>,
    /// The asset blinding factor the signer blinds with, instead of a random one.
    pub asset_blinding_factor: Option<AssetBlindingFactor>,
    /// The value blinding factor the signer blinds with, instead of a random one.
    pub value_blinding_factor: Option<ValueBlindingFactor>,
}

impl Default for PartialOutput {
    /// An empty-script output of zero of the default asset, matching the default `UTXO`.
    fn default() -> Self {
        Self::new(Script::new(), 0, AssetId::default())
    }
}

impl PartialOutput {
    /// Creates an explicit output paying `amount` of `asset` to `script`.
    #[must_use]
    pub fn new(script: Script, amount: u64, asset: AssetId) -> Self {
        Self {
            script_pubkey: script,
            amount,
            asset,
            blinding_key: None,
            asset_blinding_factor: None,
            value_blinding_factor: None,
        }
    }

    /// Creates a zero-value `OP_RETURN` output carrying `data`.
    #[must_use]
    pub fn new_metadata(data: &[u8]) -> Self {
        Self {
            script_pubkey: Script::new_op_return(data),
            amount: 0,
            asset: AssetId::default(),
            blinding_key: None,
            asset_blinding_factor: None,
            value_blinding_factor: None,
        }
    }

    /// Sets the blinding public key, so that the signer blinds this output.
    #[must_use]
    pub fn with_blinding_key(mut self, blinding_key: PublicKey) -> Self {
        self.blinding_key = Some(blinding_key);

        self
    }

    /// Blinds this output's asset with `asset_blinding_factor` instead of a random one.
    ///
    /// The output needs a blinding key, and the factor must differ from the one any input brings
    /// the same asset in with. A covenant can check the factor, and Elements reads a reissuance
    /// token's asset factor as the next reissuance's nonce.
    #[must_use]
    pub fn with_asset_blinding_factor(mut self, asset_blinding_factor: AssetBlindingFactor) -> Self {
        self.asset_blinding_factor = Some(asset_blinding_factor);

        self
    }

    /// Blinds this output's amount with `value_blinding_factor` instead of a random one.
    ///
    /// The output needs a blinding key. The signer solves one blinded output's value factor so
    /// that the transaction balances, so at least one blinded output, normally the change, must
    /// leave this unset.
    #[must_use]
    pub fn with_value_blinding_factor(mut self, value_blinding_factor: ValueBlindingFactor) -> Self {
        self.value_blinding_factor = Some(value_blinding_factor);

        self
    }

    /// Converts this output into a PSET output.
    #[must_use]
    pub fn to_output(&self) -> Output {
        let mut output = Output::new_explicit(self.script_pubkey.clone(), self.amount, self.asset, self.blinding_key);

        // the index doesn't really matter as we are the only signer
        if self.blinding_key.is_some() {
            output.blinder_index = Some(0);
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_output_is_zero_value_and_unblinded() {
        let output = PartialOutput::new_metadata(b"hello");

        assert_eq!(output.amount, 0);
        assert_eq!(output.asset, AssetId::default());
        assert!(output.blinding_key.is_none());
    }
}
