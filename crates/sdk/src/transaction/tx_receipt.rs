use std::fmt;
use std::fmt::{Debug, Display, Formatter};

use simplicityhl::elements::Txid;

use crate::provider::{ProviderError, ProviderTrait};

/// The ID of a broadcast transaction, with the provider it was broadcast through.
///
/// `Display` and `Debug` print the transaction ID.
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
    /// Creates a receipt for `tx_id`, broadcast through `provider`.
    pub fn new(provider: &'a dyn ProviderTrait, tx_id: Txid) -> Self {
        Self { provider, tx_id }
    }

    /// Returns the ID of the broadcasted transaction.
    #[must_use]
    pub fn txid(self) -> Txid {
        self.tx_id
    }

    /// Blocks until the transaction has one confirmation, using [`ProviderTrait::wait`].
    ///
    /// # Errors
    /// Returns `ProviderError::Confirmation` if the transaction doesn't confirm before the provider
    /// stops waiting, or another `ProviderError` if a request fails.
    #[inline]
    pub fn wait(&self) -> Result<(), ProviderError> {
        self.provider.wait(&self.tx_id)
    }
}
