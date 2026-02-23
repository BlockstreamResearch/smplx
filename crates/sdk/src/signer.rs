use elements_miniscript::Descriptor;
use elements_miniscript::bitcoin::PublicKey;
use elements_miniscript::descriptor::Wpkh;
use simplicityhl::elements::Address;
use simplicityhl::elements::pset::{Output, PartiallySignedTransaction};
use simplicityhl::elements::secp256k1_zkp::All;
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, Keypair, Message, Secp256k1, schnorr::Signature};
use simplicityhl::elements::{Transaction, TxOut};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::hashes::Hash;

use bip39::Mnemonic;
use elements_miniscript::{
    DescriptorPublicKey,
    bitcoin::{
        NetworkKind, PrivateKey,
        bip32::DerivationPath,
        sign_message::{MessageSignature, MessageSignatureError},
    },
    elements::{
        EcdsaSighashType,
        bitcoin::bip32::{Fingerprint, Xpriv, Xpub},
        sighash::SighashCache,
    },
    elementssig_to_rawsig,
    psbt::PsbtExt,
};
use std::str::FromStr;

use crate::constants::SimplicityNetwork;
use crate::error::SimplexError;
use crate::program::ProgramTrait;

pub trait SignerTrait {
    fn sign_program(
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
    network: SimplicityNetwork,
    secp: Secp256k1<All>,
}

impl SignerTrait for Signer {
    fn sign_program(
        &self,
        program: &dyn ProgramTrait,
        tx: &Transaction,
        utxos: &[TxOut],
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<Signature, SimplexError> {
        let env = program.get_env(tx, utxos, input_index, network)?;
        let msg = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        let private_key = self
            .get_private_key()
            .map_err(|e| SimplexError::WitnessSatisfaction(format!("{e:?}")))?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(self.secp.sign_schnorr(&msg, &keypair))
    }
}

impl Signer {
    pub const SEED_LEN: usize = secp256k1::constants::SECRET_KEY_SIZE;

    pub fn new(mnemonic: &str, network: SimplicityNetwork) -> Result<Self, String> {
        let secp = Secp256k1::new();
        let mnemonic: Mnemonic = mnemonic.parse().map_err(|e| format!("{e:?}"))?;
        let seed = mnemonic.to_seed("");

        let xprv = Xpriv::new_master(NetworkKind::Test, &seed).map_err(|e| format!("{e:?}"))?;

        Ok(Self {
            xprv,
            network: network,
            secp: secp,
        })
    }

    pub fn get_wpkh_address(&self) -> Result<Address, String> {
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

    pub fn get_schnorr_public_key(&self) -> Result<XOnlyPublicKey, String> {
        let private_key = self.get_private_key()?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(keypair.x_only_public_key().0)
    }

    pub fn get_ecdsa_public_key(&self) -> Result<PublicKey, String> {
        Ok(self.get_private_key()?.public_key(&self.secp))
    }

    pub fn get_private_key(&self) -> Result<PrivateKey, String> {
        let master_xprv = self.master_xpriv()?;
        let full_path = self.get_derivation_path()?;

        let derived =
            full_path.extend(DerivationPath::from_str("0/1").map_err(|e| format!("Derivation error: {e:?}"))?);

        let ext_derived = master_xprv
            .derive_priv(&self.secp, &derived)
            .map_err(|e| format!("Derivation error: {e:?}"))?;

        Ok(PrivateKey::new(ext_derived.private_key, NetworkKind::Test))
    }

    pub fn sign_pst(&self, pst: &PartiallySignedTransaction) -> Result<PartiallySignedTransaction, String> {
        let mut signed_pst = pst.clone();
        let tx = signed_pst.extract_tx().map_err(|e| format!("Sighash error: {e:?}"))?;

        let mut sighash_cache = SighashCache::new(&tx);
        let mut messages = vec![];

        let genesis_hash = elements_miniscript::elements::BlockHash::all_zeros();

        for (i, _) in signed_pst.inputs().iter().enumerate() {
            messages.push(
                signed_pst
                    .sighash_msg(i, &mut sighash_cache, None, genesis_hash)
                    .map_err(|e| format!("Sighash error: {e:?}"))?
                    .to_secp_msg(),
            )
        }

        for (input, msg) in signed_pst.inputs_mut().iter_mut().zip(messages) {
            let private_key = self.get_private_key()?;
            let public_key = private_key.public_key(&self.secp);

            let sig = self.secp.sign_ecdsa_low_r(&msg, &private_key.inner);
            let sig = elementssig_to_rawsig(&(sig, EcdsaSighashType::All));

            input.partial_sigs.insert(public_key, sig);
        }

        Ok(signed_pst)
    }

    fn derive_xpriv(&self, path: &DerivationPath) -> Result<Xpriv, String> {
        Ok(self.xprv.derive_priv(&self.secp, &path).map_err(|e| format!("{e:?}"))?)
    }

    fn master_xpriv(&self) -> Result<Xpriv, String> {
        Ok(self
            .derive_xpriv(&DerivationPath::master())
            .map_err(|e| format!("{e:?}"))?)
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, String> {
        let derived = self.derive_xpriv(path)?;

        Ok(Xpub::from_priv(&self.secp, &derived))
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
            "exist carry drive collect lend cereal occur much tiger just involve mean",
            SimplicityNetwork::Liquid,
        )
        .unwrap();

        let address = signer.get_wpkh_address().unwrap();
        let pk = address.script_pubkey();

        println!("{address}");
        println!("{pk}");
    }
}
