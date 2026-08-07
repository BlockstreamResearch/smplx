use elements_miniscript::bitcoin::PublicKey;

use simplicityhl::elements::Script;

/// Where a transaction's change should go, supplied by the caller.
/// 
/// Without this the signer would send change to the single address it derives internally.
#[derive(Debug, Clone)]
pub struct ChangeOutput {
    /// The script the change output pays to.
    pub script_pubkey: Script,
    /// The blinding public key, when the change output is confidential.
    pub blinding_key: Option<PublicKey>,
}

impl ChangeOutput {
    /// Creates an explicit (unblinded) change target.
    #[must_use]
    pub fn new(script_pubkey: Script) -> Self {
        Self {
            script_pubkey,
            blinding_key: None,
        }
    }

    /// Attaches a blinding public key, making the change output confidential.
    #[must_use]
    pub fn with_blinding_key(mut self, blinding_key: PublicKey) -> Self {
        self.blinding_key = Some(blinding_key);

        self
    }
}
