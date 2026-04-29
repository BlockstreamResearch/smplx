use crate::signer::{Signer, SignerError, SignerTrait};
use crate::transaction::UTXO;
use simplicityhl::elements;
use simplicityhl::elements::OutPoint;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Transaction(elements::Transaction);

#[derive(thiserror::Error, Debug)]
pub enum TransactionError {
    #[error("Incorrect vout index, get: {vout_index}, vouts in tx: {vout_len}")]
    IncorrectVoutIndex { vout_index: usize, vout_len: usize },
    #[error(transparent)]
    SignerError(#[from] SignerError),
}

impl From<elements::Transaction> for Transaction {
    fn from(txid: elements::Transaction) -> Self {
        Transaction(txid)
    }
}

impl From<Transaction> for elements::Transaction {
    fn from(value: Transaction) -> Self {
        value.0
    }
}

impl AsRef<elements::Transaction> for Transaction {
    fn as_ref(&self) -> &elements::Transaction {
        &self.0
    }
}

impl Transaction {
    #[inline]
    pub fn get_explicit_out(&self, vout: u32) -> Result<UTXO, TransactionError> {
        self.get_utxo_out(vout)
    }

    #[inline]
    pub fn get_unblinded_out<S: SignerTrait + ?Sized>(
        self,
        signer: &Signer,
        vout: u32,
    ) -> Result<UTXO, TransactionError> {
        let utxo = self.get_utxo_out(vout)?;
        Ok(signer.unblind(vec![utxo])?[0].clone())
    }

    pub fn into_inner(self) -> elements::Transaction {
        self.0
    }
}

impl Transaction {
    #[inline]
    fn get_utxo_out(&self, vout: u32) -> Result<UTXO, TransactionError> {
        let vout_usize = vout as usize;
        if vout_usize >= self.0.output.len() {
            return Err(TransactionError::IncorrectVoutIndex {
                vout_index: vout_usize,
                vout_len: self.0.output.len(),
            });
        }
        Ok(UTXO {
            outpoint: OutPoint::new(self.0.txid(), vout),
            txout: self.0.output[vout_usize].clone(),
            secrets: None,
        })
    }
}
