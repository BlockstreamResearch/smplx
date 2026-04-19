use std::sync::Arc;

use lwk_simplicity::wallet_abi::KeyStoreMeta;
use lwk_wollet::elements::pset::PartiallySignedTransaction;
use lwk_wollet::secp256k1::schnorr::Signature;
use lwk_wollet::secp256k1::{Message, XOnlyPublicKey};

use super::WalletAbiAdapterError;
use crate::wallet_abi::state::WalletAbiSharedState;

#[derive(Clone)]
pub(crate) struct WalletAbiSignerAdapter {
    shared: Arc<WalletAbiSharedState>,
}

impl WalletAbiSignerAdapter {
    pub(crate) fn new(shared: Arc<WalletAbiSharedState>) -> Self {
        Self { shared }
    }
}

impl KeyStoreMeta for WalletAbiSignerAdapter {
    type Error = WalletAbiAdapterError;

    fn get_raw_signing_x_only_pubkey(&self) -> Result<XOnlyPublicKey, Self::Error> {
        self.shared.signer_xonly()
    }

    fn sign_pst(&self, pst: &mut PartiallySignedTransaction) -> Result<(), Self::Error> {
        self.shared.sign_pst(pst)
    }

    fn sign_schnorr(&self, message: Message, xonly_public_key: XOnlyPublicKey) -> Result<Signature, Self::Error> {
        let (keypair, expected_xonly) = self.shared.signing_keypair()?;
        if xonly_public_key != expected_xonly {
            return Err(WalletAbiAdapterError::XOnlyKeyMismatch);
        }
        Ok(keypair.sign_schnorr(message))
    }
}
