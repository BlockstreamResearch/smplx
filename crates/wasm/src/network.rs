use smplx_sdk::provider::SimplicityNetwork;

use wasm_bindgen::prelude::*;

/// Resolves a network name to the Simplex SDK's network enum.
pub(crate) fn network_from_str(network: &str) -> Result<SimplicityNetwork, JsError> {
    match network {
        "liquid" => Ok(SimplicityNetwork::Liquid),
        "liquid-testnet" | "liquidtestnet" => Ok(SimplicityNetwork::LiquidTestnet),
        "elements-regtest" | "elementsregtest" | "regtest" => Ok(SimplicityNetwork::default_regtest()),
        other => Err(JsError::new(&format!("Unknown network: {other}"))),
    }
}
