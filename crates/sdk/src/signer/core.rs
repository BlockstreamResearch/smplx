use std::collections::{HashMap, HashSet};
use std::str::FromStr;

use simplicityhl::Value;
use simplicityhl::WitnessValues;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::secp256k1_zkp::{All, Keypair, Message, Secp256k1, ecdsa, schnorr};
use simplicityhl::elements::{Address, AssetId, OutPoint, Script, Transaction, Txid};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::simplicity::hashes::Hash;
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;

use bip39::Mnemonic;
use bip39::rand::thread_rng;

use elements_miniscript::{
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey,
    bitcoin::{NetworkKind, PrivateKey, PublicKey, bip32::DerivationPath},
    elements::{
        EcdsaSighashType,
        bitcoin::bip32::{Fingerprint, Xpriv, Xpub},
        sighash::SighashCache,
    },
    elementssig_to_rawsig,
    psbt::PsbtExt,
    slip77::MasterBlindingKey,
};

use crate::constants::MIN_FEE;
use crate::program::ProgramTrait;
use crate::provider::ProviderTrait;
use crate::provider::SimplicityNetwork;
use crate::transaction::{FinalTransaction, PartialInput, PartialOutput, RequiredSignature, UTXO};

use super::error::SignerError;

pub const PLACEHOLDER_FEE: u64 = 1;

pub trait SignerTrait {
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<schnorr::Signature, SignerError>;

    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
    ) -> Result<(PublicKey, ecdsa::Signature), SignerError>;
}

pub struct Signer {
    mnemonic: Mnemonic,
    xprv: Xpriv,
    provider: Box<dyn ProviderTrait>,
    network: SimplicityNetwork,
    secp: Secp256k1<All>,
}

impl SignerTrait for Signer {
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        input_index: usize,
        network: &SimplicityNetwork,
    ) -> Result<schnorr::Signature, SignerError> {
        let env = program.get_env(pst, input_index, network)?;
        let msg = Message::from_digest(env.c_tx_env().sighash_all().to_byte_array());

        let private_key = self.get_private_key()?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(self.secp.sign_schnorr(&msg, &keypair))
    }

    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
    ) -> Result<(PublicKey, ecdsa::Signature), SignerError> {
        let tx = pst.extract_tx()?;

        let mut sighash_cache = SighashCache::new(&tx);
        let genesis_hash = elements_miniscript::elements::BlockHash::all_zeros();

        let message = pst
            .sighash_msg(input_index, &mut sighash_cache, None, genesis_hash)?
            .to_secp_msg();

        let private_key = self.get_private_key()?;
        let public_key = private_key.public_key(&self.secp);

        let signature = self.secp.sign_ecdsa_low_r(&message, &private_key.inner);

        Ok((public_key, signature))
    }
}

enum Estimate {
    Success(Transaction, u64),
    Failure(u64),
}

impl Signer {
    pub fn new(mnemonic: &str, provider: Box<dyn ProviderTrait>) -> Result<Self, SignerError> {
        let secp = Secp256k1::new();
        let mnemonic: Mnemonic = mnemonic
            .parse()
            .map_err(|e: bip39::Error| SignerError::Mnemonic(e.to_string()))?;
        let seed = mnemonic.to_seed("");
        let xprv = Xpriv::new_master(NetworkKind::Test, &seed)?;

        let network = *provider.get_network();

        Ok(Self {
            mnemonic,
            xprv,
            provider,
            network,
            secp,
        })
    }

    // TODO: add an ability to send arbitrary assets
    pub fn send(&self, to: Script, amount: u64) -> Result<Txid, SignerError> {
        let mut ft = FinalTransaction::new();

        ft.add_output(PartialOutput::new(to, amount, self.network.policy_asset()));

        let (tx, _fee) = self.finalize(&ft)?;

        Ok(self.provider.broadcast_transaction(&tx)?)
    }

    pub fn broadcast(&self, tx: &FinalTransaction) -> Result<Txid, SignerError> {
        let (tx, _fee) = self.finalize(tx)?;

        Ok(self.provider.broadcast_transaction(&tx)?)
    }

    pub fn finalize(&self, tx: &FinalTransaction) -> Result<(Transaction, u64), SignerError> {
        let mut signer_utxos = self.get_utxos_asset(self.network.policy_asset())?;
        let mut set = HashSet::new();

        for input in tx.inputs() {
            set.insert(OutPoint {
                txid: input.partial_input.witness_txid,
                vout: input.partial_input.witness_output_index,
            });
        }

        signer_utxos.retain(|utxo| !set.contains(&utxo.outpoint));

        // descending sort of both confidential and explicit utxos
        signer_utxos.sort_by(|a, b| {
            let a_value = match a.secrets {
                Some(secrets) => secrets.value,
                None => a.txout.value.explicit().unwrap(),
            };
            let b_value = match b.secrets {
                Some(secrets) => secrets.value,
                None => b.txout.value.explicit().unwrap(),
            };

            b_value.cmp(&a_value)
        });

        let mut fee_tx = tx.clone();
        let mut curr_fee = MIN_FEE;
        let fee_rate = self.provider.fetch_fee_rate(1)?;

        for utxo in signer_utxos {
            let policy_amount_delta = fee_tx.calculate_fee_delta(&self.network);

            if policy_amount_delta >= curr_fee as i64 {
                match self.estimate_tx(fee_tx.clone(), fee_rate, policy_amount_delta as u64)? {
                    Estimate::Success(tx, fee) => return Ok((tx, fee)),
                    Estimate::Failure(required_fee) => curr_fee = required_fee,
                }
            }

            fee_tx.add_input(PartialInput::new(utxo), RequiredSignature::NativeEcdsa)?;
        }

        // need to try one more time after the loop
        let policy_amount_delta = fee_tx.calculate_fee_delta(&self.network);

        if policy_amount_delta >= curr_fee as i64 {
            match self.estimate_tx(fee_tx.clone(), fee_rate, policy_amount_delta as u64)? {
                Estimate::Success(tx, fee) => return Ok((tx, fee)),
                Estimate::Failure(required_fee) => curr_fee = required_fee,
            }
        }

        Err(SignerError::NotEnoughFunds(curr_fee))
    }

    pub fn finalize_strict(
        &self,
        tx: &FinalTransaction,
        target_blocks: u32,
    ) -> Result<(Transaction, u64), SignerError> {
        let policy_amount_delta = tx.calculate_fee_delta(&self.network);

        if policy_amount_delta < MIN_FEE as i64 {
            return Err(SignerError::DustAmount(policy_amount_delta));
        }

        let fee_rate = self.provider.fetch_fee_rate(target_blocks)?;

        // policy_amount_delta will be > 0
        match self.estimate_tx(tx.clone(), fee_rate, policy_amount_delta as u64)? {
            Estimate::Success(tx, fee) => Ok((tx, fee)),
            Estimate::Failure(required_fee) => Err(SignerError::NotEnoughFeeAmount(policy_amount_delta, required_fee)),
        }
    }

    pub fn get_provider(&self) -> &dyn ProviderTrait {
        self.provider.as_ref()
    }

    pub fn get_confidential_address(&self) -> Result<Address, SignerError> {
        let mut descriptor = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(&self.get_slip77_descriptor()?)
            .map_err(|e| SignerError::Slip77Descriptor(e.to_string()))?;

        // confidential descriptor doesn't support multipath
        descriptor.descriptor = descriptor.descriptor.into_single_descriptors()?[0].clone();

        Ok(descriptor
            .at_derivation_index(1)?
            .address(&self.secp, self.network.address_params())?)
    }

    pub fn get_address(&self) -> Result<Address, SignerError> {
        let descriptor = Descriptor::<DescriptorPublicKey>::from_str(&self.get_wpkh_descriptor()?)
            .map_err(|e| SignerError::WpkhDescriptor(e.to_string()))?;

        Ok(descriptor.into_single_descriptors()?[0]
            .at_derivation_index(1)?
            .address(self.network.address_params())?)
    }

    pub fn get_utxos(&self) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|_| true, &|_| true)
    }

    pub fn get_utxos_asset(&self, asset: AssetId) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|utxo| utxo.txout.asset.explicit().unwrap() == asset, &|utxo| {
            utxo.secrets.unwrap().asset == asset
        })
    }

    // TODO: can this be optimized to not populate TxOuts that are filtered out?
    pub fn get_utxos_txid(&self, txid: Txid) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|utxo| utxo.outpoint.txid == txid, &|utxo| utxo.outpoint.txid == txid)
    }

    pub fn get_utxos_filter(
        &self,
        explicit_filter: &dyn Fn(&UTXO) -> bool,
        confidential_filter: &dyn Fn(&UTXO) -> bool,
    ) -> Result<Vec<UTXO>, SignerError> {
        // fetch explicit and confidential utxos
        let mut all_utxos = self.provider.fetch_address_utxos(&self.get_confidential_address()?)?;

        // filter out only confidential utxos and unblind them
        let mut confidential_utxos = self.unblind(
            all_utxos
                .iter()
                .filter(|utxo| utxo.txout.value.is_confidential())
                .cloned()
                .collect(),
        )?;
        // leave only explicit utxos
        all_utxos.retain(|utxo| !utxo.txout.value.is_confidential());

        all_utxos.retain(explicit_filter);
        confidential_utxos.retain(confidential_filter);

        // push unblinded utxos to explicit ones
        all_utxos.extend(confidential_utxos);

        Ok(all_utxos)
    }

    pub fn get_schnorr_public_key(&self) -> Result<XOnlyPublicKey, SignerError> {
        let private_key = self.get_private_key()?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(keypair.x_only_public_key().0)
    }

    pub fn get_ecdsa_public_key(&self) -> Result<PublicKey, SignerError> {
        Ok(self.get_private_key()?.public_key(&self.secp))
    }

    pub fn get_blinding_public_key(&self) -> Result<PublicKey, SignerError> {
        Ok(self.get_blinding_private_key()?.public_key(&self.secp))
    }

    pub fn get_private_key(&self) -> Result<PrivateKey, SignerError> {
        let master_xprv = self.master_xpriv()?;
        let full_path = self.get_derivation_path()?;

        let derived =
            full_path.extend(DerivationPath::from_str("0/1").map_err(|e| SignerError::DerivationPath(e.to_string()))?);

        let ext_derived = master_xprv.derive_priv(&self.secp, &derived)?;

        Ok(PrivateKey::new(ext_derived.private_key, NetworkKind::Test))
    }

    pub fn get_blinding_private_key(&self) -> Result<PrivateKey, SignerError> {
        let blinding_key = self
            .master_slip77()?
            .blinding_private_key(&self.get_address()?.script_pubkey());

        Ok(PrivateKey::new(blinding_key, NetworkKind::Test))
    }

    fn unblind(&self, utxos: Vec<UTXO>) -> Result<Vec<UTXO>, SignerError> {
        let mut unblinded: Vec<UTXO> = Vec::new();

        for mut utxo in utxos {
            let blinding_key = self.get_blinding_private_key()?;
            let secrets = utxo.txout.unblind(&self.secp, blinding_key.inner)?;

            utxo.secrets = Some(secrets);

            unblinded.push(utxo);
        }

        Ok(unblinded)
    }

    fn estimate_tx(
        &self,
        mut fee_tx: FinalTransaction,
        fee_rate: f32,
        available_delta: u64,
    ) -> Result<Estimate, SignerError> {
        // estimate the tx fee with the change
        // use this wpkh address as a change script
        // TODO: this should be confidential
        fee_tx.add_output(PartialOutput::new(
            self.get_address()?.script_pubkey(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
        ));

        fee_tx.add_output(PartialOutput::new(
            Script::new(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
        ));

        let final_tx = self.sign_tx(&fee_tx)?;
        let fee = fee_tx.calculate_fee(final_tx.discount_weight(), fee_rate);

        if available_delta > fee && available_delta - fee >= MIN_FEE {
            // we have enough funds to cover the change UTXO
            let outputs = fee_tx.outputs_mut();

            outputs[outputs.len() - 2].amount = available_delta - fee;
            outputs[outputs.len() - 1].amount = fee;

            let final_tx = self.sign_tx(&fee_tx)?;

            return Ok(Estimate::Success(final_tx, fee));
        }

        // not enough funds, so we need to estimate without the change
        fee_tx.remove_output(fee_tx.n_outputs() - 2);

        let final_tx = self.sign_tx(&fee_tx)?;
        let fee = fee_tx.calculate_fee(final_tx.discount_weight(), fee_rate);

        if available_delta < fee {
            return Ok(Estimate::Failure(fee));
        }

        let outputs = fee_tx.outputs_mut();

        // change the fee output amount
        outputs[outputs.len() - 1].amount = available_delta;

        // finalize the tx with fee and without the change
        let final_tx = self.sign_tx(&fee_tx)?;

        Ok(Estimate::Success(final_tx, fee))
    }

    fn sign_tx(&self, tx: &FinalTransaction) -> Result<Transaction, SignerError> {
        let (mut pst, secrets) = tx.extract_pst();
        let inputs = tx.inputs();

        if tx.needs_blinding() {
            pst.blind_last(&mut thread_rng(), &self.secp, &secrets)?;
        }

        for (index, input_i) in inputs.iter().enumerate() {
            // we need to prune the program
            if let Some(program_input) = &input_i.program_input {
                let signed_witness: Result<WitnessValues, SignerError> = match &input_i.required_sig {
                    // sign the program and insert the signature into the witness
                    RequiredSignature::Witness(witness_name) => Ok(self.get_signed_program_witness(
                        &pst,
                        program_input.program.as_ref(),
                        &program_input.witness.build_witness(),
                        witness_name,
                        index,
                    )?),
                    // just build the passed witness
                    _ => Ok(program_input.witness.build_witness()),
                };
                let pruned_witness = program_input
                    .program
                    .finalize(&pst, &signed_witness.unwrap(), index, &self.network)
                    .unwrap();

                pst.inputs_mut()[index].final_script_witness = Some(pruned_witness);
            } else {
                // we need to sign the UTXO as is
                // TODO: do we always sign?
                let signed_witness = self.sign_input(&pst, index)?;
                let raw_sig = elementssig_to_rawsig(&(signed_witness.1, EcdsaSighashType::All));

                pst.inputs_mut()[index].final_script_witness = Some(vec![raw_sig, signed_witness.0.to_bytes()]);
            }
        }

        Ok(pst.extract_tx()?)
    }

    fn get_signed_program_witness(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        witness: &WitnessValues,
        witness_name: &str,
        index: usize,
    ) -> Result<WitnessValues, SignerError> {
        let signature = self.sign_program(pst, program, index, &self.network)?;

        let mut hm = HashMap::new();

        witness.iter().for_each(|el| {
            hm.insert(el.0.clone(), el.1.clone());
        });

        hm.insert(
            WitnessName::from_str_unchecked(witness_name),
            Value::byte_array(signature.serialize()),
        );

        Ok(WitnessValues::from(hm))
    }

    fn master_slip77(&self) -> Result<MasterBlindingKey, SignerError> {
        let seed = self.mnemonic.to_seed("");

        Ok(MasterBlindingKey::from_seed(&seed[..]))
    }

    fn derive_xpriv(&self, path: &DerivationPath) -> Result<Xpriv, SignerError> {
        Ok(self.xprv.derive_priv(&self.secp, &path)?)
    }

    fn master_xpriv(&self) -> Result<Xpriv, SignerError> {
        self.derive_xpriv(&DerivationPath::master())
    }

    fn derive_xpub(&self, path: &DerivationPath) -> Result<Xpub, SignerError> {
        let derived = self.derive_xpriv(path)?;

        Ok(Xpub::from_priv(&self.secp, &derived))
    }

    fn master_xpub(&self) -> Result<Xpub, SignerError> {
        self.derive_xpub(&DerivationPath::master())
    }

    fn fingerprint(&self) -> Result<Fingerprint, SignerError> {
        Ok(self.master_xpub()?.fingerprint())
    }

    fn get_slip77_descriptor(&self) -> Result<String, SignerError> {
        let wpkh_descriptor = self.get_wpkh_descriptor()?;
        let blinding_key = self.master_slip77()?;

        Ok(format!("ct(slip77({blinding_key}),{wpkh_descriptor})"))
    }

    fn get_wpkh_descriptor(&self) -> Result<String, SignerError> {
        let fingerprint = self.fingerprint()?;
        let path = self.get_derivation_path()?;
        let xpub = self.derive_xpub(&path)?;

        Ok(format!("elwpkh([{fingerprint}/{path}]{xpub}/<0;1>/*)"))
    }

    fn get_derivation_path(&self) -> Result<DerivationPath, SignerError> {
        let coin_type = if self.network.is_mainnet() { 1776 } else { 1 };
        let path = format!("84h/{coin_type}h/0h");

        DerivationPath::from_str(&format!("m/{path}")).map_err(|e| SignerError::DerivationPath(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::provider::EsploraProvider;

    use super::*;

    fn create_signer() -> Signer {
        let url = "https://blockstream.info/liquidtestnet/api".to_string();
        let network = SimplicityNetwork::LiquidTestnet;

        Signer::new(
            "exist carry drive collect lend cereal occur much tiger just involve mean",
            Box::new(EsploraProvider::new(url, network)),
        )
        .unwrap()
    }

    #[test]
    fn keys_correspond_to_address() {
        let signer = create_signer();

        let address = signer.get_address().unwrap();
        let pubkey = signer.get_ecdsa_public_key().unwrap();

        let derived_addr = Address::p2wpkh(&pubkey, None, signer.get_provider().get_network().address_params());

        assert_eq!(derived_addr.to_string(), address.to_string());
    }

    #[test]
    fn descriptors() {
        let signer = create_signer();

        println!("{}", signer.get_address().unwrap());
        println!("{}", signer.get_confidential_address().unwrap());
    }
}
