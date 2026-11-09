use smplx_sdk::transaction::IssuanceDetails as SdkIssuanceDetails;

use wasm_bindgen::prelude::*;

/// Asset issuance details.
#[wasm_bindgen]
pub struct IssuanceDetails {
    asset_id: String,
    asset_entropy: String,
    inflation_asset_id: String,
}

#[wasm_bindgen]
impl IssuanceDetails {
    pub(crate) fn from_details(details: &SdkIssuanceDetails) -> Self {
        Self {
            asset_id: details.asset_id.to_string(),
            asset_entropy: details.asset_entropy.to_string(),
            inflation_asset_id: details.inflation_asset_id.to_string(),
        }
    }

    /// The asset this issuance creates.
    #[wasm_bindgen(getter, js_name = assetId)]
    #[must_use]
    pub fn asset_id(&self) -> String {
        self.asset_id.clone()
    }

    /// The asset entropy both IDs derive from, which a later reissuance needs.
    #[wasm_bindgen(getter, js_name = assetEntropy)]
    #[must_use]
    pub fn asset_entropy(&self) -> String {
        self.asset_entropy.clone()
    }

    /// The reissuance token this issuance creates, which a later reissuance must spend.
    #[wasm_bindgen(getter, js_name = inflationAssetId)]
    #[must_use]
    pub fn inflation_asset_id(&self) -> String {
        self.inflation_asset_id.clone()
    }
}
