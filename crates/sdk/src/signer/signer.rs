use std::collections::HashMap;
use std::str::FromStr;

use elements_miniscript::Descriptor;
use elements_miniscript::bitcoin::PublicKey;
use elements_miniscript::descriptor::Wpkh;
use simplicityhl::Value;
use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::secp256k1_zkp::{self as secp256k1, All, Keypair, Message, Secp256k1, ecdsa, schnorr};
use simplicityhl::elements::{Address, Script, Transaction};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::hashes::Hash;
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;

use bip39::Mnemonic;

use elements_miniscript::{
    DescriptorPublicKey,
    bitcoin::{NetworkKind, PrivateKey, bip32::DerivationPath},
    elements::{
        EcdsaSighashType,
        bitcoin::bip32::{Fingerprint, Xpriv, Xpub},
        sighash::SighashCache,
    },
    elementssig_to_rawsig,
    psbt::PsbtExt,
};

use crate::constants::{MIN_FEE, PLACEHOLDER_FEE, SimplicityNetwork};
use crate::error::SimplexError;
use crate::program::program::ProgramTrait;
use crate::provider::Provider;
use crate::transaction::final_transaction::FinalTransaction;
use crate::transaction::final_transaction::RequiredSignature;
use crate::transaction::partial_output::PartialOutput;

pub trait SignerTrait {
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &Box<dyn ProgramTrait>,
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<schnorr::Signature, SimplexError>;

    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
    ) -> Result<(PublicKey, ecdsa::Signature), SimplexError>;
}

pub struct Signer {
    xprv: Xpriv,
    provider: Box<dyn Provider>,
    network: SimplicityNetwork,
    secp: Secp256k1<All>,
}

impl SignerTrait for Signer {
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &Box<dyn ProgramTrait>,
        input_index: usize,
        network: SimplicityNetwork,
    ) -> Result<schnorr::Signature, SimplexError> {
        let env = program.get_env(&pst, input_index, network)?;
        let msg = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        let private_key = self
            .get_private_key()
            .map_err(|e| SimplexError::SigningFailed(format!("{e:?}")))?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(self.secp.sign_schnorr(&msg, &keypair))
    }

    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
    ) -> Result<(PublicKey, ecdsa::Signature), SimplexError> {
        let tx = pst.extract_tx()?;

        let mut sighash_cache = SighashCache::new(&tx);
        let genesis_hash = elements_miniscript::elements::BlockHash::all_zeros();

        let message = pst
            .sighash_msg(input_index, &mut sighash_cache, None, genesis_hash)
            .map_err(|e| SimplexError::SigningFailed(format!("{e:?}")))?
            .to_secp_msg();

        let private_key = self
            .get_private_key()
            .map_err(|e| SimplexError::SigningFailed(format!("{e:?}")))?;
        let public_key = private_key.public_key(&self.secp);

        let signature = self.secp.sign_ecdsa_low_r(&message, &private_key.inner);

        Ok((public_key, signature))
    }
}

impl Signer {
    pub const SEED_LEN: usize = secp256k1::constants::SECRET_KEY_SIZE;

    pub fn new(mnemonic: &str, provider: Box<dyn Provider>, network: SimplicityNetwork) -> Result<Self, String> {
        let secp = Secp256k1::new();
        let mnemonic: Mnemonic = mnemonic.parse().map_err(|e| format!("{e:?}"))?;
        let seed = mnemonic.to_seed("");

        let xprv = Xpriv::new_master(NetworkKind::Test, &seed).map_err(|e| format!("{e:?}"))?;

        Ok(Self {
            xprv,
            provider: provider,
            network: network,
            secp: secp,
        })
    }

    pub fn finalize(&self, tx: &FinalTransaction, target_blocks: u32) -> Result<(Transaction, u64), SimplexError> {
        let policy_amount_delta = tx.calculate_fee_delta();

        if policy_amount_delta < MIN_FEE {
            return Err(SimplexError::DustAmount(policy_amount_delta));
        }

        // estimate the tx fee with the change
        let fee_rate = self.provider.get_fee_rate(target_blocks)?;
        let mut fee_tx = tx.clone();

        // use this wpkh address as a change script
        fee_tx.add_output(PartialOutput::new(
            self.get_wpkh_address()
                .map_err(|e| SimplexError::SigningFailed(format!("{e:?}")))?
                .script_pubkey(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
        ));

        fee_tx.add_output(PartialOutput::new(
            Script::new(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
        ));

        let final_tx = self.finalize_tx(&fee_tx)?;
        let fee = fee_tx.calculate_fee(final_tx.weight(), fee_rate);

        if policy_amount_delta > fee && policy_amount_delta - fee >= MIN_FEE {
            // we have enough funds to cover change UTXO
            let outputs = fee_tx.outputs_mut();

            outputs[outputs.len() - 2].amount = policy_amount_delta - fee;
            outputs[outputs.len() - 1].amount = fee;

            let final_tx = self.finalize_tx(&fee_tx)?;

            return Ok((final_tx, fee));
        }

        // not enough funds, so we need to estimate without the change
        fee_tx.remove_output(fee_tx.n_outputs() - 2);

        let final_tx = self.finalize_tx(&fee_tx)?;
        let fee = fee_tx.calculate_fee(final_tx.weight(), fee_rate);

        if policy_amount_delta < fee {
            return Err(SimplexError::NotEnoughFeeAmount(policy_amount_delta, fee));
        }

        let outputs = fee_tx.outputs_mut();

        // change the fee output amount
        outputs[outputs.len() - 1].amount = policy_amount_delta;

        // finalize the tx with fee and without the change
        let final_tx = self.finalize_tx(&fee_tx)?;

        Ok((final_tx, fee))
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

    fn finalize_tx(&self, tx: &FinalTransaction) -> Result<Transaction, SimplexError> {
        let mut pst = tx.extract_pst();
        let inputs = tx.inputs();

        for index in 0..inputs.len() {
            let input = inputs[index].clone();

            // we need to prune the program
            if input.program_input.is_some() {
                let program = input.program_input.unwrap();
                let signed_witness: Result<WitnessValues, SimplexError> = match input.required_sig {
                    // sign the program and insert the signature into the witness
                    RequiredSignature::Witness(witness_name) => Ok(self.get_signed_program_witness(
                        &pst,
                        &program.program,
                        &program.witness.build_witness(),
                        &witness_name,
                        index,
                    )?),
                    // just build the passed witness
                    _ => Ok(program.witness.build_witness()),
                };
                let pruned_witness = program
                    .program
                    .finalize(&pst, &signed_witness.unwrap(), index, self.network)
                    .unwrap();

                pst.inputs_mut()[index].final_script_witness = Some(pruned_witness);
            } else {
                // we need to sign the UTXO as is
                // TODO: do we always sign?
                let signed_witness = self.sign_input(&pst, index)?;
                let raw_sig = elementssig_to_rawsig(&(signed_witness.1, EcdsaSighashType::All));

                pst.inputs_mut()[index].partial_sigs.insert(signed_witness.0, raw_sig);
            }
        }

        Ok(pst.extract_tx()?)
    }

    fn get_signed_program_witness(
        &self,
        pst: &PartiallySignedTransaction,
        program: &Box<dyn ProgramTrait>,
        witness: &WitnessValues,
        witness_name: &String,
        index: usize,
    ) -> Result<WitnessValues, SimplexError> {
        let signature = self.sign_program(pst, program, index, self.network)?;

        let mut hm = HashMap::new();

        witness.iter().for_each(|el| {
            hm.insert(el.0.clone(), el.1.clone());
        });

        hm.insert(
            WitnessName::from_str_unchecked(witness_name.as_str()),
            Value::byte_array(signature.serialize()),
        );

        Ok(WitnessValues::from(hm))
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
        //     let signer = Signer::new(
        //         "exist carry drive collect lend cereal occur much tiger just involve mean",
        //         SimplicityNetwork::Liquid,
        //     )
        //     .unwrap();

        //     let address = signer.get_wpkh_address().unwrap();
        //     let pk = address.script_pubkey();

        //     println!("{address}");
        //     println!("{pk}");
    }
}
