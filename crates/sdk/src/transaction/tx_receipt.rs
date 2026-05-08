use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use simplicityhl::elements::Txid;

use crate::provider::{ProviderError, ProviderTrait};

/// A receipt for a broadcast transaction, containing the provider context and the transaction ID.
#[derive(Clone, Copy)]
pub struct TxReceipt<'a> {
    provider: &'a dyn ProviderTrait,
    tx_id: Txid,
}

impl Display for TxReceipt<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.tx_id, f)
    }
}

impl Debug for TxReceipt<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.tx_id, f)
    }
}

impl AsRef<Txid> for TxReceipt<'_> {
    fn as_ref(&self) -> &Txid {
        &self.tx_id
    }
}

impl<'a> TxReceipt<'a> {
    /// Creates a new `TxReceipt` associated with a specific provider and transaction ID.
    pub fn new(provider: &'a dyn ProviderTrait, tx_id: Txid) -> Self {
        Self { provider, tx_id }
    }

    /// Returns the ID of the broadcasted transaction.
    #[must_use]
    pub fn txid(self) -> Txid {
        self.tx_id
    }

    /// Blocks and waits for the transaction to be confirmed by the provider.
    ///
    /// # Errors
    /// Returns a `ProviderError` if the provider encounters an error while tracking the transaction state.
    #[inline]
    pub fn wait(&self) -> Result<(), ProviderError> {
        self.provider.wait(&self.tx_id)
    }
}
