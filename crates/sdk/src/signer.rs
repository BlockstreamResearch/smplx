use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair, Message, schnorr::Signature};
use simplicityhl::elements::{Transaction, TxOut};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::hashes::Hash as _;

use crate::constants::SimplicityNetwork;
use crate::error::SignerError;
use crate::program::{ProgramTrait};

pub trait SignerTrait {
    fn public_key(&self) -> XOnlyPublicKey;

    fn personal_sign(&self, message: Message) -> Result<Signature, SignerError>;

    fn sign<'a>(
        &self,
        program: &dyn ProgramTrait,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Signature, SignerError>;
}

pub struct Signer {
    keypair: Keypair,
}

impl SignerTrait for Signer {
    fn public_key(&self) -> XOnlyPublicKey {
        self.keypair.x_only_public_key().0
    }

    fn personal_sign(&self, message: Message) -> Result<Signature, SignerError> {
        Ok(self.keypair.sign_schnorr(message))
    }

    fn sign<'a>(
        &self,
        program: &dyn ProgramTrait,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Signature, SignerError> {
        let env = program.get_env(tx, utxos, input_index, network)?;

        let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        Ok(self.keypair.sign_schnorr(sighash_all))
    }
}

impl Signer {
    pub const SEED_LEN: usize = secp256k1::constants::SECRET_KEY_SIZE;

    pub fn from_seed(seed: &[u8; Self::SEED_LEN]) -> Result<Self, SignerError> {
        let secp = secp256k1::Secp256k1::new();

        let secret_key = secp256k1::SecretKey::from_slice(seed)?;

        let keypair = Keypair::from_secret_key(&secp, &secret_key);

        Ok(Self { keypair })
    }
}
