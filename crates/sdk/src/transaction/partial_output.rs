use simplicityhl::elements::pset::Output;
use simplicityhl::elements::{AssetId, Script};

#[derive(Clone)]
pub struct PartialOutput {
    pub script_pubkey: Script,
    pub amount: u64,
    pub asset: AssetId,
}

impl PartialOutput {
    pub fn new(script: Script, amount: u64, asset: AssetId) -> Self {
        Self {
            script_pubkey: script,
            amount: amount,
            asset: asset,
        }
    }

    pub fn to_output(&self) -> Output {
        Output::new_explicit(self.script_pubkey.clone(), self.amount, self.asset.clone(), None)
    }
}
