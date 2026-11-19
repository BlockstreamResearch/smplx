use std::sync::Arc;

use simplicityhl::elements::Transaction;
use simplicityhl::simplicity::ffi::{CElementsTxEnv, CFrameItem};
use simplicityhl::simplicity::jet::elements::ElementsEnv;
use simplicityhl::simplicity::jet::{Elements, JetEnvironment};

/// An Elements environment in which every signature check passes.
///
/// A signature check either aborts the program or returns nothing, so passing every check keeps
/// the branches the spend takes. Used to measure a spend, never to sign one.
pub(crate) struct UnverifiedEnv<'env>(pub(crate) &'env ElementsEnv<Arc<Transaction>>);

impl JetEnvironment for UnverifiedEnv<'_> {
    type Jet = Elements;
    type CJetEnvironment = CElementsTxEnv;

    fn c_jet_env(&self) -> &CElementsTxEnv {
        self.0.c_jet_env()
    }

    fn c_jet_ptr(jet: &Elements) -> fn(&mut CFrameItem, CFrameItem, &CElementsTxEnv) -> bool {
        match jet {
            Elements::Bip0340Verify | Elements::CheckSigVerify => Self::pass,
            _ => ElementsEnv::<Arc<Transaction>>::c_jet_ptr(jet),
        }
    }
}

impl UnverifiedEnv<'_> {
    fn pass(_: &mut CFrameItem, _: CFrameItem, _: &CElementsTxEnv) -> bool {
        true
    }
}
