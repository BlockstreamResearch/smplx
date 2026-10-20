use simplicityhl::elements::Script;

use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::FinalTransaction;
use smplx_sdk::utils::random_mnemonic;

use crate::fuzz::execution::SignedTransaction;
use crate::fuzz::outcome::SetupError;

/// Checks that an accepted spend depends on the outputs it locks.
pub(crate) struct LockCheck<'a> {
    signer: &'a Signer,
    transaction: &'a FinalTransaction,
    locked_outputs: Vec<(usize, &'a str)>,
}

impl<'a> LockCheck<'a> {
    /// `None` when no output is locked. Each locked output comes with the program it is locked to.
    pub(crate) fn new(
        signer: &'a Signer,
        transaction: &'a FinalTransaction,
        locked_outputs: Vec<(usize, &'a str)>,
    ) -> Option<Self> {
        (!locked_outputs.is_empty()).then_some(Self {
            signer,
            transaction,
            locked_outputs,
        })
    }

    /// The bypasses found, empty when every redirected output is rejected.
    ///
    /// # Errors
    /// Returns a [`SetupError`] when a variant cannot be signed or executed.
    pub(crate) fn run(&self) -> Result<Vec<String>, SetupError> {
        let attacker_script = self.attacker_script();
        let mut bypasses = Vec::new();

        for &(index, name) in &self.locked_outputs {
            if self.accepts(&self.redirected(index, &attacker_script))? {
                bypasses.push(format!("output {index} (locked to `{name}`) can be redirected"));
            }
        }

        Ok(bypasses)
    }

    fn attacker_script(&self) -> Script {
        Signer::from_mnemonic(&random_mnemonic(), *self.signer.get_network())
            .get_address()
            .script_pubkey()
    }

    fn redirected(&self, index: usize, script: &Script) -> FinalTransaction {
        let mut transaction = self.transaction.clone();
        transaction.outputs_mut()[index].script_pubkey = script.clone();

        transaction
    }

    /// Signatures are redone, so a valid signature cannot hide a missing output check.
    fn accepts(&self, transaction: &FinalTransaction) -> Result<bool, SetupError> {
        SignedTransaction::new(self.signer, transaction)?.accepts_all()
    }
}
