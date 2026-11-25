use wasm_bindgen::prelude::*;

/// The version of the Simplex SDK compiled into this module.
#[wasm_bindgen(js_name = sdkVersion)]
#[must_use]
pub fn sdk_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
