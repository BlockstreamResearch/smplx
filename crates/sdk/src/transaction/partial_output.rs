use elements_miniscript::bitcoin::PublicKey;

use simplicityhl::elements::pset::Output;
use simplicityhl::elements::{AssetId, Script};

#[derive(Debug, Clone)]
pub struct PartialOutput {
    pub script_pubkey: Script,
    pub amount: u64,
    pub asset: AssetId,
    pub blinding_key: Option<PublicKey>,
}

impl PartialOutput {
    pub fn new(script: Script, amount: u64, asset: AssetId) -> Self {
        Self {
            script_pubkey: script,
            amount,
            asset,
            blinding_key: None,
        }
    }

    pub fn new_metadata(data: &[u8]) -> Self {
        Self {
            script_pubkey: Script::new_op_return(data),
            amount: 0,
            asset: AssetId::default(),
            blinding_key: None,
        }
    }

    pub fn with_blinding_key(mut self, blinding_key: PublicKey) -> Self {
        self.blinding_key = Some(blinding_key);

        self
    }

    pub fn to_output(&self) -> Output {
        let mut output = Output::new_explicit(self.script_pubkey.clone(), self.amount, self.asset, self.blinding_key);

        // the index doesn't really matter as we are the only signer
        if self.blinding_key.is_some() {
            output.blinder_index = Some(0);
        }

        output
    }
}
