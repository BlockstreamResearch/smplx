use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Deref;
use std::str::FromStr;
use std::sync::Arc;

use simplicityhl::Value;
use simplicityhl::elements::pset::PartiallySignedTransaction;
use simplicityhl::elements::secp256k1_zkp::{All, Keypair, Message, Secp256k1, ecdsa, schnorr};
use simplicityhl::elements::{Address, LockTime, Script, Sequence, Transaction, Witness};
#[cfg(feature = "provider")]
use simplicityhl::elements::{AssetId, OutPoint, Txid};
use simplicityhl::simplicity::bitcoin::XOnlyPublicKey;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{TemplateProgramWitness, WitnessNameToValueMap, WitnessValues};

use bip39::Mnemonic;
use bip39::rand::thread_rng;

use elements_miniscript::{
    ConfidentialDescriptor, Descriptor, DescriptorPublicKey,
    bitcoin::{
        PrivateKey, PublicKey,
        bip32::{ChildNumber, DerivationPath},
    },
    elements::{
        EcdsaSighashType,
        bitcoin::bip32::{Fingerprint, Xpriv, Xpub},
        sighash::SighashCache,
    },
    elementssig_to_rawsig,
    psbt::PsbtExt,
    slip77::MasterBlindingKey,
};

use crate::constants::{DUST_THRESHOLD, MIN_FEE};
use crate::program::ProgramTrait;
use crate::program::logger::ProgramLogger;
#[cfg(feature = "provider")]
use crate::provider::ProviderTrait;
use crate::provider::SimplicityNetwork;
use crate::signer::witness_injector::WitnessInjector;
use crate::transaction::{
    ChangeOutput, FinalTransaction, PartialOutput, ProgramSignature, RequiredSignature, SigMessage,
};
#[cfg(feature = "provider")]
use crate::transaction::{NativeSignature, PartialInput, TxReceipt, UTXO};

use super::error::SignerError;

/// A placeholder dummy fee amount used during transaction estimation.
pub const PLACEHOLDER_FEE: u64 = 1;

/// Discounted weight of a native input with its signature.
#[cfg(feature = "provider")]
const NATIVE_INPUT_WEIGHT: usize = 274;

/// Weight units per virtual byte.
const WITNESS_SCALE_FACTOR: usize = 4;

/// Common signing interface spanning over standard explicit inputs and Simplicity programs.
pub trait SignerTrait {
    /// Generates a Schnorr signature to satisfy a target Simplicity program input.
    ///
    /// # Errors
    /// Returns a `SignerError` if the elements environment fails to build or if the message digest fails to construct.
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        input_index: usize,
        network: &SimplicityNetwork,
        derivation_path: Option<&DerivationPath>,
        message: &SigMessage,
    ) -> Result<schnorr::Signature, SignerError>;

    /// Generates an ECDSA signature to spend a standard transaction input.
    ///
    /// # Errors
    /// Returns a `SignerError` if the transaction formatting or sighash msg extraction fails.
    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        derivation_path: Option<&DerivationPath>,
    ) -> Result<(PublicKey, ecdsa::Signature), SignerError>;
}

/// Core interface responsible for managing keys, interfacing with the blockchain provider,
/// assembling descriptors, estimating fees, and finalizing/signing transactions.
///
/// Without the `provider` feature the signer has no blockchain access: it can assemble,
/// blind, sign and finalize a transaction it is handed, but it cannot discover UTXOs,
/// look up a fee rate, or broadcast.
pub struct Signer {
    #[cfg(feature = "provider")]
    provider: Option<Box<dyn ProviderTrait>>,
    network: SimplicityNetwork,
    secp: Secp256k1<All>,
    /// The account key at `m/84h/{coin}h/0h`, derived once from the seed.
    account_xprv: Xpriv,
    /// The signing key at the default path `0/0` under the account.
    default_key: PrivateKey,
    address: Address,
    confidential_address: Address,
    blinding_key: PrivateKey,
}

impl SignerTrait for Signer {
    fn sign_program(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        input_index: usize,
        network: &SimplicityNetwork,
        derivation_path: Option<&DerivationPath>,
        message: &SigMessage,
    ) -> Result<schnorr::Signature, SignerError> {
        let env = program.get_env(pst, input_index, network)?;
        let sighash = env.c_tx_env().sighash_all().to_byte_array();
        let msg = Message::from_digest(message.digest(sighash));

        let private_key = self.get_private_key_at(derivation_path)?;
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        Ok(self.secp.sign_schnorr(&msg, &keypair))
    }

    fn sign_input(
        &self,
        pst: &PartiallySignedTransaction,
        input_index: usize,
        derivation_path: Option<&DerivationPath>,
    ) -> Result<(PublicKey, ecdsa::Signature), SignerError> {
        let tx = pst.extract_tx()?;

        self.sign_input_with(pst, &mut SighashCache::new(&tx), input_index, derivation_path)
    }
}

enum Estimate {
    Success(Transaction, u64),
    Failure(u64),
}

// TODO: refactor descriptors to be a standalone object to specify custom derivation paths.
impl Signer {
    /// Creates a new `Signer` instance seeded from the provided mnemonic and paired with the specified provider.
    ///
    /// # Panics
    /// Panics if the mnemonic fails to parse, or if deriving the master private key fails.
    #[cfg(feature = "provider")]
    #[must_use]
    pub fn new(mnemonic: &str, provider: Box<dyn ProviderTrait>) -> Self {
        let network = *provider.get_network();
        let mut signer = Self::from_mnemonic(mnemonic, network);

        signer.provider = Some(provider);

        signer
    }

    /// Creates a `Signer` from a mnemonic and an explicit network, with no blockchain access.
    ///
    /// This is the constructor a host with its own networking and its own key custody should use.
    ///
    /// # Panics
    /// Panics if the mnemonic fails to parse, or if deriving the master private key fails.
    #[must_use]
    pub fn from_mnemonic(mnemonic: &str, network: SimplicityNetwork) -> Self {
        let secp = Secp256k1::new();
        let mnemonic: Mnemonic = mnemonic
            .parse()
            .unwrap_or_else(|error: bip39::Error| panic!("{}", SignerError::Mnemonic(error.to_string())));

        let seed = mnemonic.to_seed("");
        let master_xprv = Xpriv::new_master(network, &seed).expect("a BIP-39 seed derives a master key");
        let master_blinding_key = MasterBlindingKey::from_seed(&seed[..]);

        let account_path = Self::account_path(network);
        let account_xprv = master_xprv
            .derive_priv(&secp, &account_path)
            .expect("the account path derives from the master key");
        let default_key = Self::derive_key(&secp, &account_xprv, &Self::default_path(), network);

        let wpkh_descriptor = Self::wpkh_descriptor(&secp, &master_xprv, &account_path, &account_xprv);
        let address = Self::wpkh_address(&wpkh_descriptor, network);
        let confidential_address = Self::confidential_address(&secp, &wpkh_descriptor, master_blinding_key, network);
        let blinding_key = PrivateKey::new(
            master_blinding_key.blinding_private_key(&address.script_pubkey()),
            network,
        );

        Self {
            #[cfg(feature = "provider")]
            provider: None,
            network,
            secp,
            account_xprv,
            default_key,
            address,
            confidential_address,
            blinding_key,
        }
    }

    /// Composes, funds, and broadcasts a standard network transaction sending the specified value of the primary policy asset.
    ///
    /// # Errors
    /// Returns a `SignerError` if compiling the inputs fails, there are insufficient funds/fees, or broadcast is rejected.
    // TODO: add an ability to send arbitrary assets
    #[cfg(feature = "provider")]
    pub fn send(&self, to: Script, amount: u64) -> Result<TxReceipt<'_>, SignerError> {
        let mut ft = FinalTransaction::new();

        ft.add_output(PartialOutput::new(to, amount, self.network.policy_asset()));

        let (tx, _fee) = self.finalize(&ft)?;

        Ok(self.get_provider()?.broadcast_transaction(&tx)?)
    }

    /// Evaluates, funds, and broadcasts an already assembled `FinalTransaction`.
    ///
    /// # Errors
    /// Returns a `SignerError` if finalizing the payload fails or if the network rejects the broadcast.
    #[cfg(feature = "provider")]
    pub fn broadcast(&self, tx: &FinalTransaction) -> Result<TxReceipt<'_>, SignerError> {
        let (tx, _fee) = self.finalize(tx)?;

        Ok(self.get_provider()?.broadcast_transaction(&tx)?)
    }

    /// Evaluates the input components of a `FinalTransaction`, iteratively selecting available wallet UTXOs to cover outputs and estimated fees.
    ///
    /// # Errors
    /// Returns a `SignerError` if the wallet contains insufficient funds to satisfy output values and target fee rates.
    #[cfg(feature = "provider")]
    pub fn finalize(&self, tx: &FinalTransaction) -> Result<(Transaction, u64), SignerError> {
        Self::check_known_amounts(tx)?;
        Self::check_unique_inputs(tx)?;

        let fee_rate = self.get_provider()?.fetch_fee_rate(1)?;
        Self::check_fee_rate(fee_rate)?;

        let mut signer_utxos = self.get_utxos_asset(self.network.policy_asset())?;
        let mut spent: HashSet<OutPoint> = tx.inputs().iter().map(|input| input.partial_input.outpoint).collect();

        // Skips the outpoints the transaction spends already and the ones the provider lists twice
        signer_utxos.retain(|utxo| spent.insert(utxo.outpoint));

        // An explicit change target can't balance a confidential input unless another output is confidential
        if tx.change().is_some_and(|change| change.blinding_key.is_none()) && !tx.needs_blinding() {
            // Retain only explicit UTXOs
            signer_utxos.retain(UTXO::is_explicit);
        }

        // Resolve amounts before sorting so an unreadable UTXO returns an error.
        let mut signer_utxos = signer_utxos
            .into_iter()
            .map(|utxo| utxo.amount().map(|amount| (amount, utxo)))
            .collect::<Result<Vec<_>, _>>()?;
        signer_utxos.sort_by_key(|(amount, _)| std::cmp::Reverse(*amount));

        let mut fee_tx = tx.clone();
        let mut curr_fee = MIN_FEE;

        // Each input added after an estimate makes the transaction heavier, so
        // the next estimate also accounts for the fee of those inputs.
        let input_fee = Self::calculate_fee(NATIVE_INPUT_WEIGHT, fee_rate);
        let mut inputs_since_estimate: u64 = 0;

        let try_estimate = |fee_tx: &FinalTransaction, policy_amount_delta: i64, curr_fee: &mut u64| match self
            .estimate_tx(fee_tx.clone(), fee_rate, policy_amount_delta.cast_unsigned())
        {
            Ok(Estimate::Success(tx, fee)) => {
                ProgramLogger::flush_logs();
                Ok(Some((tx, fee)))
            }
            Ok(Estimate::Failure(required_fee)) => {
                *curr_fee = required_fee;
                Ok(None)
            }
            Err(err) => {
                ProgramLogger::flush_logs();
                Err(err)
            }
        };

        for (_, utxo) in signer_utxos {
            let policy_amount_delta = fee_tx.calculate_fee_delta(&self.network)?;
            let required_fee = curr_fee.saturating_add(inputs_since_estimate.saturating_mul(input_fee));

            if i128::from(policy_amount_delta) >= i128::from(required_fee) {
                inputs_since_estimate = 0;

                if let Some(result) = try_estimate(&fee_tx, policy_amount_delta, &mut curr_fee)? {
                    return Ok(result);
                }
            }

            // IMPORTANT: must be added to the end of the transaction.
            // Otherwise, the logging of execution traces will be broken
            fee_tx.add_input(PartialInput::new(utxo), Some(NativeSignature::Ecdsa));
            inputs_since_estimate += 1;
        }

        // Need to try one more time after the loop. No UTXO is left to add, so any chance is taken
        let policy_amount_delta = fee_tx.calculate_fee_delta(&self.network)?;

        if i128::from(policy_amount_delta) >= i128::from(curr_fee) {
            inputs_since_estimate = 0;

            if let Some(result) = try_estimate(&fee_tx, policy_amount_delta, &mut curr_fee)? {
                return Ok(result);
            }
        }

        Err(SignerError::NotEnoughFunds(
            curr_fee.saturating_add(inputs_since_estimate.saturating_mul(input_fee)),
        ))
    }

    /// Verifies and finalizes a transaction against a strict target confirmation window (in blocks).
    /// This function also assumes that the transaction already includes the coin selection.
    ///
    /// # Errors
    /// Returns a `SignerError` if the assembled inputs do not meet dust limits or fail to cover the
    /// dynamically estimated required fee.
    pub fn finalize_strict(&self, tx: &FinalTransaction, fee_rate: f32) -> Result<(Transaction, u64), SignerError> {
        Self::check_known_amounts(tx)?;
        Self::check_unique_inputs(tx)?;
        Self::check_fee_rate(fee_rate)?;

        let policy_amount_delta = tx.calculate_fee_delta(&self.network)?;

        if policy_amount_delta < MIN_FEE.cast_signed() {
            return Err(SignerError::DustAmount(policy_amount_delta));
        }

        // policy_amount_delta will be > 0
        match self.estimate_tx(tx.clone(), fee_rate, policy_amount_delta.cast_unsigned())? {
            Estimate::Success(tx, fee) => {
                ProgramLogger::flush_logs();
                Ok((tx, fee))
            }
            Estimate::Failure(required_fee) => Err(SignerError::NotEnoughFeeAmount(policy_amount_delta, required_fee)),
        }
    }

    /// Reports what an assembled transaction would cost in fees at the given rate.
    ///
    /// # Errors
    /// Returns a `SignerError` if the transaction cannot be signed.
    pub fn estimate_fee(&self, tx: &FinalTransaction, fee_rate: f32) -> Result<u64, SignerError> {
        Self::check_known_amounts(tx)?;
        Self::check_unique_inputs(tx)?;
        Self::check_fee_rate(fee_rate)?;

        let available_delta = tx.calculate_fee_delta(&self.network)?.max(0).cast_unsigned();
        let estimate = self.estimate_tx(tx.clone(), fee_rate, available_delta);

        ProgramLogger::flush_logs();

        Ok(match estimate? {
            Estimate::Success(_, fee) | Estimate::Failure(fee) => fee,
        })
    }

    /// Signs transaction in raw format for easy processing later in a format of `PartiallySignedTransaction`.
    ///
    /// Every program input is executed and finalized with its signed witness.
    ///
    /// # Errors
    /// Returns a `SignerError` if we have an error in singing and constructing program witness,
    /// or if a program fails to execute.
    pub fn sign_tx(&self, tx: &FinalTransaction) -> Result<PartiallySignedTransaction, SignerError> {
        let (mut pst, signed_witnesses) = self.sign_witnesses(tx)?;

        for (index, input_i) in tx.inputs().iter().enumerate() {
            let Some(program_input) = &input_i.program_input else {
                continue;
            };

            let pruned_witness = program_input
                .program
                .finalize(&pst, &signed_witnesses[&index], index, &self.network)
                .map_err(|source| SignerError::CovenantExecution {
                    index,
                    locktime: pst.locktime().map_or(0, LockTime::to_consensus_u32),
                    sequence: pst.inputs()[index]
                        .sequence
                        .map_or(u32::MAX, Sequence::to_consensus_u32),
                    source,
                })?;

            pst.inputs_mut()[index].final_script_witness = Some(Witness::from(pruned_witness));
        }

        Ok(pst)
    }

    /// Blinds the transaction, signs the `NativeEcdsa` inputs and injects signatures into program witnesses,
    /// without executing or finalizing the programs. Inputs that require no signature stay unsigned.
    ///
    /// # Errors
    /// Returns a `SignerError` if blinding, signing or injecting a signature into a program witness fails.
    pub fn sign_witnesses(
        &self,
        tx: &FinalTransaction,
    ) -> Result<(PartiallySignedTransaction, HashMap<usize, WitnessValues>), SignerError> {
        Self::check_known_amounts(tx)?;

        let (mut pst, secrets) = tx.extract_pst()?;
        let inputs = tx.inputs();
        let mut signed_witnesses = HashMap::new();

        if tx.needs_blinding() {
            pst.blind_last(&mut thread_rng(), &self.secp, &secrets)?;
        }

        // Native inputs share one extracted transaction and sighash cache.
        let mut sighash_cache = if inputs
            .iter()
            .any(|input| matches!(input.required_sig, RequiredSignature::NativeEcdsa))
        {
            Some(SighashCache::new(Box::new(pst.extract_tx()?)))
        } else {
            None
        };

        for (index, input_i) in inputs.iter().enumerate() {
            if let Some(program_input) = &input_i.program_input {
                let signed_witness = match &input_i.required_sig {
                    // Sign the program and inject the signature into the witness
                    RequiredSignature::Program(signature) => self.get_signed_program_witness(
                        &pst,
                        program_input.program.as_ref(),
                        &program_input.witness,
                        signature,
                        index,
                        input_i.partial_input.derivation_path.as_ref(),
                    )?,
                    // Just build the witness
                    _ => program_input.witness.shallow_clone(),
                };

                signed_witnesses.insert(index, signed_witness);
            } else if matches!(input_i.required_sig, RequiredSignature::NativeEcdsa)
                && let Some(sighash_cache) = sighash_cache.as_mut()
            {
                // An input that requires no signature is left for its owner to satisfy
                let signed_witness = self.sign_input_with(
                    &pst,
                    sighash_cache,
                    index,
                    input_i.partial_input.derivation_path.as_ref(),
                )?;
                let raw_sig = elementssig_to_rawsig(&(signed_witness.1, EcdsaSighashType::All));

                pst.inputs_mut()[index].final_script_witness =
                    Some(Witness::from(vec![raw_sig, signed_witness.0.to_bytes()]));
            }
        }

        Ok((pst, signed_witnesses))
    }

    /// Returns the network used to derive keys and finalize transactions.
    #[must_use]
    pub const fn get_network(&self) -> &SimplicityNetwork {
        &self.network
    }

    /// Returns a reference to the active configured network provider.
    ///
    /// # Errors
    /// Returns `ProviderUnavailable` when the signer was built without one.
    #[cfg(feature = "provider")]
    pub fn get_provider(&self) -> Result<&dyn ProviderTrait, SignerError> {
        self.provider.as_deref().ok_or(SignerError::ProviderUnavailable)
    }

    /// Returns the confidential elements address matching the local wallet logic.
    #[must_use]
    pub fn get_confidential_address(&self) -> Address {
        self.confidential_address.clone()
    }

    /// Returns the standard unblinded address matching the local wallet logic.
    #[must_use]
    pub fn get_address(&self) -> Address {
        self.address.clone()
    }

    /// Retrieves UTXOs with known assets and amounts, skipping outputs that cannot be unblinded.
    ///
    /// # Errors
    /// Returns a `SignerError` if querying the provider fails.
    #[cfg(feature = "provider")]
    pub fn get_utxos(&self) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|_| true, &|_| true)
    }

    /// Finds usable UTXOs belonging to the specific `AssetId`, skipping unreadable outputs.
    ///
    /// # Errors
    /// Returns a `SignerError` if querying the provider fails.
    #[cfg(feature = "provider")]
    pub fn get_utxos_asset(&self, asset: AssetId) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|utxo| utxo.asset().is_ok_and(|id| id == asset), &|utxo| {
            utxo.asset().is_ok_and(|id| id == asset)
        })
    }

    /// Finds all known UTXOs deriving from a targeted `Txid`.
    ///
    /// # Errors
    /// Returns a `SignerError` if querying the network fails.
    // TODO: can this be optimized to not populate TxOuts that are filtered out?
    #[cfg(feature = "provider")]
    pub fn get_utxos_txid(&self, txid: Txid) -> Result<Vec<UTXO>, SignerError> {
        self.get_utxos_filter(&|utxo| utxo.outpoint.txid == txid, &|utxo| utxo.outpoint.txid == txid)
    }

    /// Maps UTXOs retrieved from the provider through arbitrary functional filters.
    /// Separate filtering criteria apply explicitly vs confidentially.
    ///
    /// Outputs with null fields, unresolved partial commitments, or proofs that
    /// cannot be rewound with the wallet key are skipped. Already supplied secrets
    /// are preserved. A single unreadable payment must not block usable funds.
    ///
    /// # Errors
    /// Returns a `SignerError` if retrieving remote outputs fails.
    #[cfg(feature = "provider")]
    pub fn get_utxos_filter(
        &self,
        explicit_filter: &dyn Fn(&UTXO) -> bool,
        confidential_filter: &dyn Fn(&UTXO) -> bool,
    ) -> Result<Vec<UTXO>, SignerError> {
        // Fetch explicit and confidential utxos
        let fetched_utxos = self
            .get_provider()?
            .fetch_address_utxos(&self.get_confidential_address())?;

        let mut all_utxos = Vec::new();
        let mut confidential_utxos = Vec::new();

        for mut utxo in fetched_utxos {
            if utxo.txout.asset.is_null() || utxo.txout.value.is_null() {
                continue;
            }

            if utxo.is_explicit() {
                all_utxos.push(utxo);
                continue;
            }

            if utxo.secrets.is_none() {
                let blinding_key = self.get_blinding_private_key();
                let Ok(secrets) = utxo.txout.unblind(&self.secp, blinding_key.inner) else {
                    // Payments to this script may use a foreign blinding key or
                    // an asset-only commitment. Neither can fund this wallet.
                    continue;
                };

                utxo.secrets = Some(secrets);
            }

            confidential_utxos.push(utxo);
        }

        all_utxos.retain(explicit_filter);
        confidential_utxos.retain(confidential_filter);

        all_utxos.extend(confidential_utxos);

        Ok(all_utxos)
    }

    /// Derives the X-Only public key specifically used for Schnorr and Taproot structures.
    #[must_use]
    pub fn get_schnorr_public_key(&self) -> XOnlyPublicKey {
        let private_key = self.get_private_key();
        let keypair = Keypair::from_secret_key(&self.secp, &private_key.inner);

        keypair.x_only_public_key().0
    }

    /// Resolves the standard format ECDSA public key.
    #[must_use]
    pub fn get_ecdsa_public_key(&self) -> PublicKey {
        self.get_private_key().public_key(&self.secp)
    }

    /// Resolves the corresponding blinding public key.
    #[must_use]
    pub fn get_blinding_public_key(&self) -> PublicKey {
        self.get_blinding_private_key().public_key(&self.secp)
    }

    /// Exposes the wallet's signing private key at the default path.
    #[must_use]
    pub fn get_private_key(&self) -> PrivateKey {
        self.default_key
    }

    /// Derives the signing key at a path relative to the account path. `None` defaults to `0/0`.
    ///
    /// # Errors
    /// Returns `SignerError::InvalidDerivationPath` if the path is not of the form `m/n`.
    pub fn get_private_key_at(&self, relative: Option<&DerivationPath>) -> Result<PrivateKey, SignerError> {
        match relative {
            Some(path) => {
                Self::check_derivation_path(path)?;

                Ok(Self::derive_key(&self.secp, &self.account_xprv, path, self.network))
            }
            None => Ok(self.default_key),
        }
    }

    /// The private key linked to confidential payload blinding, derived from the master SLIP77 key
    /// and the script public key of the address.
    #[must_use]
    pub fn get_blinding_private_key(&self) -> PrivateKey {
        self.blinding_key
    }

    /// Checks that `path` is in correct form of `m/n`.
    ///
    /// # Errors
    /// Returns `SignerError::InvalidDerivationPath` otherwise.
    pub fn check_derivation_path(path: &DerivationPath) -> Result<(), SignerError> {
        match path.as_ref() {
            [ChildNumber::Normal { .. }, ChildNumber::Normal { .. }] => Ok(()),
            _ => Err(SignerError::InvalidDerivationPath(path.clone())),
        }
    }

    fn sign_input_with(
        &self,
        pst: &PartiallySignedTransaction,
        sighash_cache: &mut SighashCache<impl Deref<Target = Transaction>>,
        input_index: usize,
        derivation_path: Option<&DerivationPath>,
    ) -> Result<(PublicKey, ecdsa::Signature), SignerError> {
        let message = pst
            .sighash_msg(input_index, sighash_cache, None, self.network.genesis_block_hash())?
            .to_secp_msg();

        let private_key = self.get_private_key_at(derivation_path)?;
        let public_key = private_key.public_key(&self.secp);

        let script_pubkey = pst.inputs()[input_index]
            .witness_utxo
            .as_ref()
            .map(|utxo| &utxo.script_pubkey);

        if let Some(script_pubkey) = script_pubkey.filter(|script| script.is_v0_p2wpkh())
            && *script_pubkey != Script::new_v0_wpkh(&public_key.wpubkey_hash().expect("the key is compressed"))
        {
            return Err(SignerError::KeyMismatch(input_index));
        }

        let signature = self.secp.sign_ecdsa_low_r(&message, &private_key.inner);

        Ok((public_key, signature))
    }

    fn estimate_tx(
        &self,
        mut fee_tx: FinalTransaction,
        fee_rate: f32,
        available_delta: u64,
    ) -> Result<Estimate, SignerError> {
        let sign_and_extract_tx = |signer: &Signer, tx: &FinalTransaction| -> Result<Transaction, SignerError> {
            Ok(signer.sign_tx(tx)?.extract_tx()?)
        };

        // Estimate the tx fee with the change. The caller supplies the change target
        let change = match fee_tx.change() {
            Some(target) => target.clone(),
            None => {
                ChangeOutput::new(self.get_address().script_pubkey()).with_blinding_key(self.get_blinding_public_key())
            }
        };

        // A confidential input with an explicit change target and no blinded
        // output cannot be balanced, which the node rejects with `bad-txns-in-ne-out`.
        if fee_tx.has_confidential_input() && !fee_tx.needs_blinding() && change.blinding_key.is_none() {
            return Err(SignerError::ConfidentialInputWithoutBlindedOutput);
        }

        let mut change_output = PartialOutput::new(change.script_pubkey, PLACEHOLDER_FEE, self.network.policy_asset());

        if let Some(blinding_key) = change.blinding_key {
            change_output = change_output.with_blinding_key(blinding_key);
        }

        fee_tx.add_output(change_output);

        fee_tx.add_output(PartialOutput::new(
            Script::new(),
            PLACEHOLDER_FEE,
            self.network.policy_asset(),
        ));

        let final_tx = sign_and_extract_tx(self, &fee_tx)?;
        let fee = Self::calculate_fee(final_tx.discount_weight(), fee_rate);

        let change_index = fee_tx.n_outputs() - 2;
        let change_minimum = if fee_tx.outputs()[change_index].blinding_key.is_some() {
            MIN_FEE
        } else {
            DUST_THRESHOLD
        };

        if available_delta
            .checked_sub(fee)
            .is_some_and(|change| change >= change_minimum)
        {
            // We have enough funds to cover the change UTXO
            let outputs = fee_tx.outputs_mut();

            outputs[outputs.len() - 2].amount = available_delta - fee;
            outputs[outputs.len() - 1].amount = fee;

            if !fee_tx.is_balanced()? {
                return Err(SignerError::Unbalanced);
            }

            let final_tx = sign_and_extract_tx(self, &fee_tx)?;

            return Ok(Estimate::Success(final_tx, fee));
        }

        let blinded_without_change = fee_tx
            .outputs()
            .iter()
            .enumerate()
            .filter(|(index, output)| *index != change_index && output.blinding_key.is_some())
            .count();
        // Explicit change can always be dropped.
        let change_needed = fee_tx.outputs()[change_index].blinding_key.is_some()
            && if fee_tx.has_confidential_input() {
                // Change is the only confidential output -> must stay
                blinded_without_change == 0
            } else {
                // There is another confidential output -> must stay to preserve privacy
                blinded_without_change == 1
            };

        if change_needed {
            return Ok(Estimate::Failure(fee.saturating_add(change_minimum)));
        }

        // Not enough funds for the change, so estimate without it.
        fee_tx.remove_output(change_index);

        let final_tx = sign_and_extract_tx(self, &fee_tx)?;
        let fee = Self::calculate_fee(final_tx.discount_weight(), fee_rate);

        if available_delta < fee {
            return Ok(Estimate::Failure(fee));
        }

        let outputs = fee_tx.outputs_mut();

        // Change the fee output amount
        outputs[outputs.len() - 1].amount = available_delta;

        if !fee_tx.is_balanced()? {
            return Err(SignerError::Unbalanced);
        }

        // Finalize the tx with fee and without the change, which pays the whole delta
        let final_tx = sign_and_extract_tx(self, &fee_tx)?;

        Ok(Estimate::Success(final_tx, available_delta))
    }

    /// Signs and inserts a signature into appropriate witness value.
    ///
    /// # Errors
    /// Returns a `SignerError` if signing the program fails, if the witness types cannot be
    /// retrieved from the program, if the signature's witness field is not present among the program's
    /// witness fields, or if injecting the signature into the witness value at its path fails.
    fn get_signed_program_witness(
        &self,
        pst: &PartiallySignedTransaction,
        program: &dyn ProgramTrait,
        witness: &WitnessValues,
        program_signature: &ProgramSignature,
        index: usize,
        derivation_path: Option<&DerivationPath>,
    ) -> Result<WitnessValues, SignerError> {
        let witness_name = program_signature.name();
        let sig_path = program_signature.path();
        let signature = self.sign_program(
            pst,
            program,
            index,
            &self.network,
            derivation_path,
            program_signature.message(),
        )?;

        // Inject the signature into the witness field directly if the path is not provided
        let sig_val = if sig_path.is_empty() {
            Value::byte_array(signature.serialize())
        } else {
            let witness_types = program.get_witness_types()?;
            let witness_type = witness_types
                .get(&TemplateProgramWitness::witness_from_str(witness_name))
                .ok_or(SignerError::WitnessFieldNotFound(witness_name.to_string()))?;

            #[allow(clippy::missing_panics_doc)]
            let local_witness = Arc::new(
                witness
                    .get(&TemplateProgramWitness::witness_from_str(witness_name))
                    .expect("checked above")
                    .clone(),
            );

            WitnessInjector::inject_value(
                &local_witness,
                witness_type,
                sig_path,
                Value::byte_array(signature.serialize()),
            )?
        };

        let mut hm = HashMap::new();

        witness.iter().for_each(|el| {
            hm.insert(el.0.clone(), el.1.clone());
        });

        hm.insert(TemplateProgramWitness::witness_from_str(witness_name), sig_val);

        Ok(WitnessValues::from_map(hm))
    }

    /// The fee in satoshis for `weight` at `fee_rate` sats/kvb, both rounded up.
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss
    )]
    fn calculate_fee(weight: usize, fee_rate: f32) -> u64 {
        let vsize = weight.div_ceil(WITNESS_SCALE_FACTOR);

        (vsize as f32 * fee_rate / 1000.0).ceil() as u64
    }

    /// A fee rate is a finite, non-negative number of sats/kvb.
    fn check_fee_rate(fee_rate: f32) -> Result<(), SignerError> {
        if fee_rate.is_finite() && fee_rate >= 0.0 {
            Ok(())
        } else {
            Err(SignerError::InvalidFeeRate(fee_rate))
        }
    }

    /// A transaction spending an outpoint twice can never confirm.
    fn check_unique_inputs(tx: &FinalTransaction) -> Result<(), SignerError> {
        let mut seen = HashSet::new();

        match tx
            .inputs()
            .iter()
            .find(|input| !seen.insert(input.partial_input.outpoint))
        {
            Some(input) => Err(SignerError::DuplicateInput(input.partial_input.outpoint)),
            None => Ok(()),
        }
    }

    /// Balancing and signing need the amount of every input.
    fn check_known_amounts(tx: &FinalTransaction) -> Result<(), SignerError> {
        match tx
            .inputs()
            .iter()
            .find(|input| input.partial_input.asset_and_amount().is_none())
        {
            Some(input) => Err(SignerError::UnknownInputAmount(input.partial_input.outpoint)),
            None => Ok(()),
        }
    }

    /// The account path, `m/84h/1776h/0h` on mainnet and `m/84h/1h/0h` elsewhere.
    fn account_path(network: SimplicityNetwork) -> DerivationPath {
        let coin_type = if network.is_mainnet() { 1776 } else { 1 };

        DerivationPath::from_str(&format!("m/84h/{coin_type}h/0h")).expect("the account path is valid")
    }

    /// The default signing path under the account.
    fn default_path() -> DerivationPath {
        DerivationPath::from_str("0/0").expect("the default path is valid")
    }

    /// # Panics
    /// Panics if `path` cannot be derived from `account_xprv`.
    fn derive_key(
        secp: &Secp256k1<All>,
        account_xprv: &Xpriv,
        path: &DerivationPath,
        network: SimplicityNetwork,
    ) -> PrivateKey {
        let derived = account_xprv
            .derive_priv(secp, path)
            .unwrap_or_else(|error| panic!("Cannot derive a key at `{path}`: {error}"));

        PrivateKey::new(derived.private_key, network)
    }

    fn wpkh_descriptor(
        secp: &Secp256k1<All>,
        master_xprv: &Xpriv,
        account_path: &DerivationPath,
        account_xprv: &Xpriv,
    ) -> String {
        let fingerprint = Xpub::from_priv(secp, master_xprv).fingerprint();
        let mut xpub = Xpub::from_priv(secp, account_xprv);

        // TODO: Blockstream app does this. Is this a bug?
        xpub.parent_fingerprint = Fingerprint::default();

        format!("elwpkh([{fingerprint}/{account_path}]{xpub}/<0;1>/*)")
    }

    /// # Panics
    /// Panics if the WPKH descriptor cannot be parsed or the address cannot be derived.
    fn wpkh_address(wpkh_descriptor: &str, network: SimplicityNetwork) -> Address {
        let descriptor =
            Descriptor::<DescriptorPublicKey>::from_str(wpkh_descriptor).expect("the wallet's descriptor parses");

        descriptor
            .into_single_descriptors()
            .expect("the wallet's descriptor is single-path")[0]
            .at_derivation_index(0)
            .expect("index 0 is a valid derivation index")
            .address(network.address_params())
            .expect("a WPKH descriptor has an address")
    }

    /// # Panics
    /// Panics if the SLIP77 descriptor cannot be parsed or the address cannot be derived.
    fn confidential_address(
        secp: &Secp256k1<All>,
        wpkh_descriptor: &str,
        master_blinding_key: MasterBlindingKey,
        network: SimplicityNetwork,
    ) -> Address {
        let mut descriptor = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(&format!(
            "ct(slip77({master_blinding_key}),{wpkh_descriptor})"
        ))
        .expect("the wallet's confidential descriptor parses");

        // Confidential descriptor doesn't support multipath
        descriptor.descriptor = descriptor
            .descriptor
            .into_single_descriptors()
            .expect("the wallet's descriptor is single-path")[0]
            .clone();

        descriptor
            .at_derivation_index(0)
            .expect("index 0 is a valid derivation index")
            .address(secp, network.address_params())
            .expect("a SLIP77 descriptor has an address")
    }
}

#[cfg(test)]
mod tests {
    use simplicityhl::elements::confidential::{Asset, AssetBlindingFactor, Value};
    use simplicityhl::elements::{OutPoint, TxOut, Txid};

    use simplicityhl::elements::TxOutSecrets;
    use simplicityhl::elements::confidential::ValueBlindingFactor;

    use crate::provider::{EsploraProvider, ProviderError};
    use crate::transaction::{FinalTransactionError, PartialInput, UTXO};
    use crate::utils::random_mnemonic;

    use super::*;

    const MNEMONIC: &str =
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

    struct StubProvider {
        network: SimplicityNetwork,
        utxos: Vec<UTXO>,
    }

    impl ProviderTrait for StubProvider {
        fn get_network(&self) -> &SimplicityNetwork {
            &self.network
        }

        fn fetch_address_utxos(&self, _: &Address) -> Result<Vec<UTXO>, ProviderError> {
            Ok(self.utxos.clone())
        }

        fn fetch_fee_estimates(&self) -> Result<HashMap<String, f64>, ProviderError> {
            Ok(HashMap::from([("1".to_string(), 0.1)]))
        }

        fn broadcast_transaction(&self, _: &Transaction) -> Result<TxReceipt<'_>, ProviderError> {
            unimplemented!()
        }

        fn wait(&self, _: &Txid) -> Result<(), ProviderError> {
            unimplemented!()
        }

        fn fetch_tip_height(&self) -> Result<u32, ProviderError> {
            unimplemented!()
        }

        fn fetch_tip_block_hash(&self) -> Result<String, ProviderError> {
            unimplemented!()
        }

        fn fetch_tip_timestamp(&self) -> Result<u64, ProviderError> {
            unimplemented!()
        }

        fn fetch_block_hash_at_height(&self, _: u32) -> Result<String, ProviderError> {
            unimplemented!()
        }

        fn fetch_block_txids(&self, _: &str) -> Result<Vec<Txid>, ProviderError> {
            unimplemented!()
        }

        fn fetch_transaction(&self, _: &Txid) -> Result<Transaction, ProviderError> {
            unimplemented!()
        }

        fn fetch_scripthash_utxos(&self, _: &Script) -> Result<Vec<UTXO>, ProviderError> {
            unimplemented!()
        }
    }

    fn wallet_with(utxos: impl FnOnce(&Signer) -> Vec<UTXO>) -> Signer {
        let network = SimplicityNetwork::default_regtest();
        let utxos = utxos(&Signer::from_mnemonic(MNEMONIC, network));

        Signer::new(MNEMONIC, Box::new(StubProvider { network, utxos }))
    }

    fn explicit_coin(signer: &Signer, vout: u32, amount: u64) -> UTXO {
        UTXO::explicit(
            OutPoint::new(Txid::from_byte_array([0x02; 32]), vout),
            signer.get_address().script_pubkey(),
            amount,
            signer.network.policy_asset(),
        )
    }

    fn confidential_coin(signer: &Signer, vout: u32, amount: u64) -> UTXO {
        let asset = signer.network.policy_asset();
        let spent = [TxOutSecrets::new(
            asset,
            AssetBlindingFactor::zero(),
            amount,
            ValueBlindingFactor::zero(),
        )];
        let (txout, ..) = TxOut::new_not_last_confidential(
            &mut thread_rng(),
            &signer.secp,
            amount,
            &signer.get_confidential_address(),
            asset,
            &spent,
        )
        .unwrap();

        UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array([0x02; 32]), vout),
            txout,
            secrets: None,
        }
    }

    fn create_signer() -> Signer {
        let url = "https://blockstream.info/liquidtestnet/api".to_string();
        let network = SimplicityNetwork::Liquid;

        Signer::new(random_mnemonic().as_str(), Box::new(EsploraProvider::new(url, network)))
    }

    fn confidential_input(signer: &Signer, value: u64) -> PartialInput {
        use simplicityhl::elements::confidential::{AssetBlindingFactor, ValueBlindingFactor};
        use simplicityhl::elements::{TxOut, TxOutSecrets};

        PartialInput::new(UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array([0x01; 32]), 0),
            txout: TxOut::default(),
            secrets: Some(TxOutSecrets::new(
                signer.network.policy_asset(),
                AssetBlindingFactor::zero(),
                value,
                ValueBlindingFactor::zero(),
            )),
        })
    }

    #[test]
    fn explicit_change_target_cannot_balance_a_confidential_input() {
        let signer = create_signer();
        let mut ft = FinalTransaction::new();

        ft.add_input(confidential_input(&signer, 100_000), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            50_000,
            signer.network.policy_asset(),
        ));
        ft.add_change(ChangeOutput::new(signer.get_address().script_pubkey()));

        assert!(matches!(
            signer.estimate_fee(&ft, 1.0),
            Err(SignerError::ConfidentialInputWithoutBlindedOutput)
        ));
    }

    #[test]
    fn blinded_output_lets_the_explicit_change_target_stand() {
        let signer = create_signer();
        let mut ft = FinalTransaction::new();

        ft.add_input(confidential_input(&signer, 100_000), Some(NativeSignature::Ecdsa));
        ft.add_output(
            PartialOutput::new(
                signer.get_address().script_pubkey(),
                50_000,
                signer.network.policy_asset(),
            )
            .with_blinding_key(signer.get_blinding_public_key()),
        );
        ft.add_change(ChangeOutput::new(signer.get_address().script_pubkey()));

        assert!(!matches!(
            signer.estimate_fee(&ft, 1.0),
            Err(SignerError::ConfidentialInputWithoutBlindedOutput)
        ));
    }

    #[test]
    fn default_change_target_balances_it_by_itself() {
        let signer = create_signer();
        let mut ft = FinalTransaction::new();

        ft.add_input(confidential_input(&signer, 100_000), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            50_000,
            signer.network.policy_asset(),
        ));

        assert!(!matches!(
            signer.estimate_fee(&ft, 1.0),
            Err(SignerError::ConfidentialInputWithoutBlindedOutput)
        ));
    }

    #[test]
    fn keys_correspond_to_address() {
        let signer = create_signer();

        let address = signer.get_address();
        let pubkey = signer.get_ecdsa_public_key();

        let derived_addr = Address::p2wpkh(
            &pubkey,
            None,
            signer.get_provider().unwrap().get_network().address_params(),
        );

        assert_eq!(derived_addr.to_string(), address.to_string());
    }

    #[test]
    fn descriptors() {
        let signer = create_signer();

        println!("{}", signer.get_address());
        println!("{}", signer.get_confidential_address());
    }

    #[test]
    fn input_without_known_amount_is_an_error() {
        use simplicityhl::elements::TxOut;

        let signer = create_signer();
        let outpoint = OutPoint::new(Txid::from_byte_array([0x03; 32]), 1);

        let mut ft = FinalTransaction::new();
        ft.add_input(
            PartialInput::new(UTXO {
                outpoint,
                txout: TxOut::default(),
                secrets: None,
            }),
            Some(NativeSignature::Ecdsa),
        );

        assert!(matches!(
            signer.sign_witnesses(&ft),
            Err(SignerError::UnknownInputAmount(unknown)) if unknown == outpoint
        ));
    }

    #[test]
    fn sign_witnesses_leaves_failing_programs_for_the_caller_to_execute() {
        use simplicityhl::Arguments;

        use simplicityhl::simplicity::bit_machine::ExecutionError;

        use crate::program::{Program, ProgramError};
        use crate::transaction::ProgramInput;

        let signer = create_signer();
        let program = Program::new("fn main() { assert!(false); }", Arguments::default());

        let mut ft = FinalTransaction::new();
        // A native input first, so the program sits at index 1
        ft.add_input(
            PartialInput::new(UTXO::explicit(
                OutPoint::new(Txid::from_byte_array([0x01; 32]), 0),
                Script::new(),
                1000,
                signer.network.policy_asset(),
            )),
            Some(NativeSignature::Ecdsa),
        );
        ft.add_program_input(
            PartialInput::new(UTXO::explicit(
                OutPoint::new(Txid::from_byte_array([0x02; 32]), 0),
                program.get_tr_address(&signer.network).script_pubkey(),
                1000,
                signer.network.policy_asset(),
            )),
            ProgramInput::new(Box::new(program.clone()), WitnessValues::default()),
            None,
        );

        let (pst, signed_witnesses) = signer.sign_witnesses(&ft).unwrap();

        assert_eq!(signed_witnesses.len(), 1);
        assert!(signed_witnesses.contains_key(&1));
        assert!(pst.inputs()[0].final_script_witness.is_some());
        assert!(pst.inputs()[1].final_script_witness.is_none());

        // The caller executes against the signed transaction and observes the program's own failure
        assert!(matches!(
            program.execute(&pst, &signed_witnesses[&1], 1, &signer.network),
            Err(ProgramError::Pruning(ExecutionError::JetFailed(_)))
        ));

        assert!(matches!(
            signer.sign_tx(&ft),
            Err(SignerError::CovenantExecution { index: 1, .. })
        ));
    }

    #[test]
    fn unresolved_inputs_are_rejected_before_signing() {
        let signer = create_signer();
        let network = signer.network;
        let (amount_in, amount_out, fee_rate) = (100_000, 10_000, 100.);

        let script = signer.get_address().script_pubkey();
        let funding = UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), 0),
            txout: TxOut {
                script_pubkey: script.clone(),
                ..TxOut::new_fee(amount_in, network.policy_asset())
            },
            secrets: None,
        };

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(funding.clone()), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(script.clone(), amount_out, network.policy_asset()));

        // The signer can finalize valid caller-selected inputs successfully.
        let (funded, _) = signer.finalize_strict(&ft, fee_rate).unwrap();
        assert_eq!(funded.input.len(), 1);
        funded.verify_tx_amt_proofs(&signer.secp, &[funding.txout]).unwrap();

        // Adding a malformed input.
        let mixed = UTXO {
            outpoint: OutPoint::new(Txid::from_byte_array(Default::default()), 0),
            txout: TxOut {
                asset: Asset::new_confidential(
                    &signer.secp,
                    network.policy_asset(),
                    AssetBlindingFactor::new(&mut thread_rng()),
                ),
                value: Value::Explicit(600),
                script_pubkey: script,
                ..Default::default()
            },
            secrets: None,
        };
        ft.add_input(PartialInput::new(mixed), Some(NativeSignature::Ecdsa));

        // Direct accounting reports the unresolved input's position.
        assert_eq!(
            ft.calculate_fee_delta(&network),
            Err(FinalTransactionError::MissingInputSecrets { input_index: 1 })
        );
        assert_eq!(
            ft.is_balanced(),
            Err(FinalTransactionError::MissingInputSecrets { input_index: 1 })
        );
        assert!(matches!(
            ft.extract_pst(),
            Err(FinalTransactionError::MissingInputSecrets { input_index: 1 })
        ));

        // Signer entry points preserve the error identifying the unresolved outpoint.
        for result in [
            signer.estimate_fee(&ft, 100.0).map(|_| ()),
            signer.finalize_strict(&ft, 100.0).map(|_| ()),
            signer.sign_witnesses(&ft).map(|_| ()),
            signer.sign_tx(&ft).map(|_| ()),
        ] {
            assert!(matches!(result, Err(SignerError::UnknownInputAmount(_))));
        }
    }

    #[test]
    fn coin_selection_skips_a_coin_listed_twice() {
        let signer = wallet_with(|owner| vec![explicit_coin(owner, 0, 100_000), explicit_coin(owner, 0, 100_000)]);

        let mut ft = FinalTransaction::new();
        ft.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            150_000,
            signer.network.policy_asset(),
        ));

        assert!(matches!(signer.finalize(&ft), Err(SignerError::NotEnoughFunds(_))));
    }

    #[test]
    fn explicit_change_skips_confidential_coins() {
        let signer = wallet_with(|owner| vec![explicit_coin(owner, 0, 50_000), confidential_coin(owner, 1, 100_000)]);
        let script = signer.get_address().script_pubkey();

        let mut ft = FinalTransaction::new();
        ft.add_output(PartialOutput::new(
            script.clone(),
            10_000,
            signer.network.policy_asset(),
        ));
        ft.add_change(ChangeOutput::new(script));

        let (tx, _) = signer.finalize(&ft).unwrap();

        assert_eq!(tx.input.len(), 1);
        assert_eq!(tx.input[0].previous_output.vout, 0);
    }

    #[test]
    fn invalid_fee_rates_and_duplicate_inputs_are_rejected() {
        let signer = create_signer();
        let coin = explicit_coin(&signer, 0, 100_000);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(coin.clone()), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(
            signer.get_address().script_pubkey(),
            10_000,
            signer.network.policy_asset(),
        ));

        for fee_rate in [f32::NAN, -1.0, f32::INFINITY] {
            assert!(matches!(
                signer.finalize_strict(&ft, fee_rate),
                Err(SignerError::InvalidFeeRate(_))
            ));
            assert!(matches!(
                signer.estimate_fee(&ft, fee_rate),
                Err(SignerError::InvalidFeeRate(_))
            ));
        }

        ft.add_input(PartialInput::new(coin), Some(NativeSignature::Ecdsa));

        assert!(matches!(
            signer.finalize_strict(&ft, 100.0),
            Err(SignerError::DuplicateInput(_))
        ));
    }

    #[test]
    fn leftover_too_small_for_change_is_the_fee_unless_it_reveals_a_blinded_amount() {
        let signer = create_signer();
        let policy = signer.network.policy_asset();
        let script = signer.get_address().script_pubkey();
        let coin = explicit_coin(&signer, 0, 100_000);

        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(coin.clone()), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(script.clone(), 99_970, policy));

        let (tx, fee) = signer.finalize_strict(&ft, 100.0).unwrap();

        assert_eq!(tx.output.len(), 2, "the change is dropped");
        assert_eq!((fee, tx.fee_in(policy)), (30, 30));

        // Without change, the inputs minus the fee would reveal the only blinded output
        let mut ft = FinalTransaction::new();
        ft.add_input(PartialInput::new(coin), Some(NativeSignature::Ecdsa));
        ft.add_output(PartialOutput::new(script, 99_970, policy).with_blinding_key(signer.get_blinding_public_key()));

        assert!(matches!(
            signer.finalize_strict(&ft, 100.0),
            Err(SignerError::NotEnoughFeeAmount(..))
        ));
    }

    #[test]
    fn only_wallet_paths_owning_the_input_sign_it() {
        let signer = create_signer();
        let path = |path: &str| DerivationPath::from_str(path).unwrap();

        for valid in ["0/0", "0/7", "1/3", "2/7"] {
            assert!(Signer::check_derivation_path(&path(valid)).is_ok(), "{valid}");
        }
        for invalid in ["", "m", "7", "0/7h", "84h/1h/0h/0/7"] {
            assert!(Signer::check_derivation_path(&path(invalid)).is_err(), "{invalid}");
        }

        let sign_with = |relative: &str| {
            let mut ft = FinalTransaction::new();
            ft.add_input(
                PartialInput::new(explicit_coin(&signer, 0, 100_000)).with_derivation_path(path(relative)),
                Some(NativeSignature::Ecdsa),
            );
            ft.add_output(PartialOutput::new(
                Script::new(),
                100_000,
                signer.network.policy_asset(),
            ));

            signer.sign_tx(&ft)
        };

        assert!(sign_with("0/0").is_ok());
        assert!(matches!(sign_with("0/7"), Err(SignerError::KeyMismatch(0))));
    }
}
