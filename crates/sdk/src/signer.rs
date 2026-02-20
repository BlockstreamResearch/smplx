use elements_miniscript::Descriptor;
use elements_miniscript::bitcoin::PublicKey;
use elements_miniscript::descriptor::Wpkh;
use simplicityhl::elements::Address;
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair, Message, Secp256k1, schnorr::Signature};
use simplicityhl::elements::{Transaction, TxOut};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;

use bip39::Mnemonic;
use elements_miniscript::{
    DescriptorPublicKey,
    bitcoin::{
        self, NetworkKind, PrivateKey,
        bip32::DerivationPath,
        sign_message::{MessageSignature, MessageSignatureError},
    },
    descriptor,
    elements::bitcoin::{
        Network,
        bip32::{self, Fingerprint, Xpriv, Xpub},
    },
};
use std::str::FromStr;

use crate::constants::SimplicityNetwork;
use crate::error::SimplexError;
use crate::program::ProgramTrait;

pub trait SignerTrait {
    fn public_key(&self) -> XOnlyPublicKey;

    fn personal_sign(&self, message: Message) -> Result<Signature, SimplexError>;

    fn sign<'a>(
        &self,
        program: &dyn ProgramTrait,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Signature, SimplexError>;
}

pub struct Signer {
    xprv: Xpriv,
    mnemonic: Mnemonic,
    network: SimplicityNetwork,
}

// impl SignerTrait for Signer {
//     fn public_key(&self) -> XOnlyPublicKey {
//         // self.keypair.x_only_public_key().0

//     }

//     fn personal_sign(&self, message: Message) -> Result<Signature, SimplexError> {
//         // Ok(self.keypair.sign_schnorr(message))
//     }

//     fn sign<'a>(
//         &self,
//         program: &dyn ProgramTrait,
//         tx: &Transaction,
//         utxos: &[TxOut],
//         input_index: usize,
//         network: SimplicityNetwork,
//     ) -> Result<Signature, SimplexError> {
//         // let env = program.get_env(tx, utxos, input_index, network)?;

//         // let sighash_all = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

//         // Ok(self.keypair.sign_schnorr(sighash_all))
//     }
// }

impl Signer {
    pub const SEED_LEN: usize = secp256k1::constants::SECRET_KEY_SIZE;

    pub fn new(mnemonic: &str, network: SimplicityNetwork) -> Result<Self, String> {
        let mnemonic: Mnemonic = mnemonic.parse().map_err(|e| format!("{e:?}"))?;
        let seed = mnemonic.to_seed("");

        let xprv = Xpriv::new_master(NetworkKind::Test, &seed).map_err(|e| format!("{e:?}"))?;

        Ok(Self {
            xprv,
            mnemonic: mnemonic,
            network: network,
        })
    }

    pub fn get_address(&self) -> Result<Address, String> {
        let fingerprint = self.fingerprint()?;
        let path = self.get_derivation_path()?;
        let xpub = self.derive_xpub(&path)?;

        let desc = format!("elwpkh([{fingerprint}/{path}]{xpub}/<0;1>/*)");

        println!("{desc}");

        let descriptor: Descriptor<DescriptorPublicKey> =
            Descriptor::Wpkh(Wpkh::from_str(&desc).map_err(|e| format!("{e:?}"))?);

        Ok(descriptor
            .clone()
            .into_single_descriptors()
            .map_err(|e| format!("{e:?}"))?[0]
            .at_derivation_index(1)
            .map_err(|e| format!("{e:?}"))?
            .address(self.network.address_params())
            .map_err(|e| format!("{e:?}"))?)
    }

    pub fn get_private_key(&self) -> Result<Xpriv, String> {
        let secp = Secp256k1::new();

        let master_xprv = self.master_xpriv()?;
        let full_path = self.get_derivation_path()?;

        let derived =
            full_path.extend(DerivationPath::from_str("0/1").map_err(|e| format!("Derivation error: {e:?}"))?);

        Ok(master_xprv
            .derive_priv(&secp, &derived)
            .map_err(|e| format!("Derivation error: {e:?}"))?)
    }

    fn derive_xpriv(&self, path: &DerivationPath) -> Result<Xpriv, String> {
        let secp = Secp256k1::new();

        Ok(self.xprv.derive_priv(&secp, &path).map_err(|e| format!("{e:?}"))?)
    }

    fn master_xpriv(&self) -> Result<Xpriv, String> {
        Ok(self
            .derive_xpriv(&DerivationPath::master())
            .map_err(|e| format!("{e:?}"))?)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, String> {
        let secp = Secp256k1::new();
        let derived = self.derive_xpriv(path)?;

        Ok(Xpub::from_priv(&secp, &derived))
    }

    fn master_xpub(&self) -> Result<Xpub, String> {
        Ok(self
            .derive_xpub(&DerivationPath::master())
            .map_err(|e| format!("{e:?}"))?)
    }

    fn fingerprint(&self) -> Result<Fingerprint, String> {
        Ok(self.master_xpub()?.fingerprint())
    }

    fn get_derivation_path(&self) -> Result<DerivationPath, String> {
        let coin_type = if self.network.is_mainnet() { 1776 } else { 1 };
        let path = format!("84h/{coin_type}h/0h");

        Ok(DerivationPath::from_str(&format!("m/{path}")).map_err(|e| format!("{e:?}"))?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor() {
        let signer = Signer::new(
            "prosper short ramp prepare exchange stove life snack client enough purpose fold",
            SimplicityNetwork::Liquid,
        )
        .unwrap();

        let address = signer.get_address().unwrap();

        println!("{address}");
    }
}
