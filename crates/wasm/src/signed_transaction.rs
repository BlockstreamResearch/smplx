use simplicityhl::elements::{self, Transaction};

use wasm_bindgen::prelude::*;

/// A finished transaction and the fee it pays.
#[wasm_bindgen]
pub struct SignedTransaction {
    fee_sats: u64,
    hex: String,
    txid: String,
}

#[wasm_bindgen]
impl SignedTransaction {
    pub(crate) fn new(transaction: &Transaction, fee_sats: u64) -> Self {
        Self {
            fee_sats,
            hex: elements::encode::serialize_hex(transaction),
            txid: transaction.txid().to_string(),
        }
    }

    /// The consensus-encoded transaction, ready to broadcast.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn hex(&self) -> String {
        self.hex.clone()
    }

    /// The transaction id it will have once broadcast.
    #[wasm_bindgen(getter)]
    #[must_use]
    pub fn txid(&self) -> String {
        self.txid.clone()
    }

    /// The fee it pays, in satoshis.
    #[wasm_bindgen(getter, js_name = feeSats)]
    #[must_use]
    pub fn fee_sats(&self) -> u64 {
        self.fee_sats
    }
}
