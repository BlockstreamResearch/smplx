//! Contract ABI as portable JSON: extracted once at build time so the generated
//! bindings and `include_simf!` never re-invoke the frontend.

use std::collections::HashMap;

use serde::Deserialize;
use simplicityhl::parse::ParseFromStr;
use simplicityhl::str::WitnessName;
use simplicityhl::types::ResolvedType;
use simplicityhl::{AbiMeta, Parameters, WitnessTypes};

/// Serializes an [`AbiMeta`] to the `{witness_types, parameter_types}` JSON.
pub fn abi_json(abi: &AbiMeta) -> Result<String, String> {
    serde_json::to_string(abi).map_err(|e| e.to_string())
}

#[derive(Deserialize)]
struct AbiJson {
    #[serde(default)]
    witness_types: HashMap<String, String>,
    #[serde(default)]
    parameter_types: HashMap<String, String>,
}

/// Reconstructs an [`AbiMeta`] from the JSON produced by [`abi_json`].
pub fn abi_from_json(json: &str) -> Result<AbiMeta, String> {
    let parsed: AbiJson = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(AbiMeta {
        witness_types: WitnessTypes::from(resolve_map(&parsed.witness_types)?),
        param_types: Parameters::from(resolve_map(&parsed.parameter_types)?),
    })
}

fn resolve_map(map: &HashMap<String, String>) -> Result<HashMap<WitnessName, ResolvedType>, String> {
    map.iter()
        .map(|(name, ty)| {
            let resolved = ResolvedType::parse_from_str(ty).map_err(|e| format!("type `{ty}`: {e}"))?;
            Ok((WitnessName::from_str_unchecked(name), resolved))
        })
        .collect()
}
