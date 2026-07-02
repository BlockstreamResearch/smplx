use std::str::FromStr;

use bip39::Mnemonic;
use bitcoincore_rpc::bitcoin::bip32::{DerivationPath, Fingerprint, Xpriv, Xpub};
use bitcoincore_rpc::bitcoin::key::{Keypair, Secp256k1};
use bitcoincore_rpc::bitcoin::secp256k1::{All, SecretKey};
use bitcoincore_rpc::bitcoin::{NetworkKind, PrivateKey, PublicKey, XOnlyPublicKey};

use elements_miniscript::elements::Address;
use elements_miniscript::slip77::MasterBlindingKey;
use elements_miniscript::{ConfidentialDescriptor, Descriptor, DescriptorPublicKey};

use crate::provider::SimplicityNetwork;
use crate::signer::SignerError;

/// A generalized interface for providing cryptographic keys and addresses.
///
/// This trait abstracts the origin of the wallet's keys, allowing the `Signer` to remain
/// agnostic to whether the keys are derived from a BIP39 mnemonic, a hardware wallet,
/// or a single injected secret key.
pub trait KeyOrigin {
    /// Derives the X-Only public key specifically used for Schnorr and Taproot structures.
    #[must_use]
    fn get_schnorr_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> XOnlyPublicKey;

    /// Resolves the standard format ECDSA public key.
    #[must_use]
    fn get_ecdsa_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey;

    /// Resolves the corresponding blinding public key.
    #[must_use]
    fn get_blinding_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey;

    /// Internally derives and exposes the wallet's signing active private key.
    ///
    /// # Panics
    /// Panics if the master private key or derivation path cannot be derived.
    #[must_use]
    fn get_private_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PrivateKey;

    /// Generates the private key linked to confidential payload blinding.
    ///
    /// The generated `PrivateKey` is associated with the `Test` (non-Bitcoin-mainnet) network kind.
    /// Retrieves the blinding private key derived from the master SLIP77 key and the script public key of the address.
    ///
    /// # Panics
    /// Panics if the master SLIP77 key cannot be derived.
    #[must_use]
    fn get_blinding_private_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PrivateKey;

    /// Returns the confidential elements address matching the local wallet logic.
    ///
    /// # Panics
    /// Panics if the SLIP77 descriptor cannot be generated or parsed, or if address derivation fails.
    #[must_use]
    fn get_confidential_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address;

    /// Returns the standard unblinded address matching the local wallet logic.
    ///
    /// # Panics
    /// Panics if the WPKH descriptor cannot be generated or parsed, or if address derivation fails.
    #[must_use]
    fn get_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address;
}

/// A Hierarchical Deterministic (HD) key provider based on BIP39 and SLIP77.
///
/// `HDKey` derives its key material from a standard mnemonic seed phrase.
/// It handles BIP32 derivation paths for standard transaction signing and uses
/// SLIP77 to generate deterministic blinding keys for confidential transactions
/// on the Elements/Liquid network.
pub struct HDKey {
    xprv: Xpriv,
    master_blinding: MasterBlindingKey,
}

impl HDKey {
    /// Constructs a new `HDKey` from a BIP39 mnemonic phrase.
    ///
    /// # Errors
    /// Returns a `SignerError::Mnemonic` if the provided phrase is invalid.
    pub fn new(mnemonic: &str) -> Result<Self, SignerError> {
        let mnemonic: Mnemonic = mnemonic
            .parse()
            .map_err(|e: bip39::Error| SignerError::Mnemonic(e.to_string()))?;
        let seed = mnemonic.to_seed("");
        let xprv = Xpriv::new_master(NetworkKind::Test, &seed)?;

        let master_blinding_key = MasterBlindingKey::from_seed(&seed[..]);

        Ok(Self {
            master_blinding: master_blinding_key,
            xprv,
        })
    }

    fn derive_xpriv(&self, path: &DerivationPath, secp: &Secp256k1<All>) -> Result<Xpriv, SignerError> {
        Ok(self.xprv.derive_priv(secp, path)?)
    }

    fn master_xpriv(&self, secp: &Secp256k1<All>) -> Result<Xpriv, SignerError> {
        self.derive_xpriv(&DerivationPath::master(), secp)
    }

    fn derive_xpub(&self, path: &DerivationPath, secp: &Secp256k1<All>) -> Result<Xpub, SignerError> {
        let derived = self.derive_xpriv(path, secp)?;

        Ok(Xpub::from_priv(secp, &derived))
    }

    fn master_xpub(&self, secp: &Secp256k1<All>) -> Result<Xpub, SignerError> {
        self.derive_xpub(&DerivationPath::master(), secp)
    }

    fn fingerprint(&self, secp: &Secp256k1<All>) -> Result<Fingerprint, SignerError> {
        Ok(self.master_xpub(secp)?.fingerprint())
    }

    fn get_slip77_descriptor(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Result<String, SignerError> {
        let wpkh_descriptor = self.get_wpkh_descriptor(secp, network)?;
        let blinding_key = self.master_blinding;

        Ok(format!("ct(slip77({blinding_key}),{wpkh_descriptor})"))
    }

    fn get_wpkh_descriptor(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Result<String, SignerError> {
        let fingerprint = self.fingerprint(secp)?;
        let path = self.get_derivation_path(network)?;
        let xpub = self.derive_xpub(&path, secp)?;

        Ok(format!("elwpkh([{fingerprint}/{path}]{xpub}/<0;1>/*)"))
    }

    #[allow(clippy::unused_self)]
    fn get_derivation_path(&self, network: &SimplicityNetwork) -> Result<DerivationPath, SignerError> {
        let coin_type = if network.is_mainnet() { 1776 } else { 1 };
        let path = format!("84h/{coin_type}h/0h");

        DerivationPath::from_str(&format!("m/{path}")).map_err(|e| SignerError::DerivationPath(e.to_string()))
    }
}

impl KeyOrigin for HDKey {
    fn get_schnorr_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> XOnlyPublicKey {
        let private_key = self.get_private_key(secp, network);
        let keypair = Keypair::from_secret_key(secp, &private_key.inner);

        keypair.x_only_public_key().0
    }

    fn get_ecdsa_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey {
        self.get_private_key(secp, network).public_key(secp)
    }

    fn get_blinding_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey {
        self.get_blinding_private_key(secp, network).public_key(secp)
    }

    fn get_private_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PrivateKey {
        let master_xprv = self.master_xpriv(secp).unwrap();
        let full_path = self.get_derivation_path(network).unwrap();

        let derived = full_path.extend(
            DerivationPath::from_str("0/1")
                .map_err(|e| SignerError::DerivationPath(e.to_string()))
                .unwrap(),
        );

        let ext_derived = master_xprv.derive_priv(secp, &derived).unwrap();

        PrivateKey::new(ext_derived.private_key, NetworkKind::Test)
    }

    fn get_blinding_private_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PrivateKey {
        let blinding_key = self
            .master_blinding
            .blinding_private_key(&self.get_address(secp, network).script_pubkey());

        PrivateKey::new(blinding_key, NetworkKind::Test)
    }

    fn get_confidential_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address {
        let mut descriptor = ConfidentialDescriptor::<DescriptorPublicKey>::from_str(
            &self.get_slip77_descriptor(secp, network).unwrap(),
        )
        .map_err(|e| SignerError::Slip77Descriptor(e.to_string()))
        .unwrap();

        // confidential descriptor doesn't support multipath
        descriptor.descriptor = descriptor.descriptor.into_single_descriptors().unwrap()[0].clone();

        descriptor
            .at_derivation_index(1)
            .unwrap()
            .address(secp, network.address_params())
            .unwrap()
    }

    fn get_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address {
        let descriptor = Descriptor::<DescriptorPublicKey>::from_str(&self.get_wpkh_descriptor(secp, network).unwrap())
            .map_err(|e| SignerError::WpkhDescriptor(e.to_string()))
            .unwrap();

        descriptor.into_single_descriptors().unwrap()[0]
            .at_derivation_index(1)
            .unwrap()
            .address(network.address_params())
            .unwrap()
    }
}

/// A simplified key provider powered by a single static secret key.
///
/// Unlike `HDKey` which derives paths hierarchically, `SingleKey` uses
/// exactly one `SecretKey` for all signing operations. It can optionally accept a
/// `MasterBlindingKey` to support confidential transactions and blinded addresses.
pub struct SingleKey {
    secret_key: SecretKey,
    blinding_key: Option<MasterBlindingKey>,
}

impl SingleKey {
    /// Creates a new `SingleKey`.
    ///
    /// # Arguments
    /// * `secret_key` - The base static secret key used for ECDSA and Schnorr signatures.
    /// * `blinding_key` - An optional SLIP77 master blinding key.
    #[must_use]
    pub fn new(secret_key: SecretKey, blinding_key: Option<MasterBlindingKey>) -> Self {
        Self {
            secret_key,
            blinding_key,
        }
    }
}

impl KeyOrigin for SingleKey {
    fn get_schnorr_public_key(&self, secp: &Secp256k1<All>, _network: &SimplicityNetwork) -> XOnlyPublicKey {
        let keypair = Keypair::from_secret_key(secp, &self.secret_key);
        keypair.x_only_public_key().0
    }

    fn get_ecdsa_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey {
        self.get_private_key(secp, network).public_key(secp)
    }

    fn get_blinding_public_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PublicKey {
        self.get_blinding_private_key(secp, network).public_key(secp)
    }

    fn get_private_key(&self, _secp: &Secp256k1<All>, _network: &SimplicityNetwork) -> PrivateKey {
        PrivateKey::new(self.secret_key, NetworkKind::Test)
    }

    fn get_blinding_private_key(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> PrivateKey {
        let master_blinding = self
            .blinding_key
            .expect("Blinding key is required for confidential operations");

        let script_pubkey = self.get_address(secp, network).script_pubkey();
        let blinding_key = master_blinding.blinding_private_key(&script_pubkey);

        PrivateKey::new(blinding_key, NetworkKind::Test)
    }

    fn get_confidential_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address {
        let ecdsa_pubkey = self.get_ecdsa_public_key(secp, network);
        let blinding_pubkey = self.get_blinding_public_key(secp, network);

        Address::p2wpkh(&ecdsa_pubkey, Some(blinding_pubkey.inner), network.address_params())
    }

    fn get_address(&self, secp: &Secp256k1<All>, network: &SimplicityNetwork) -> Address {
        let ecdsa_pubkey = self.get_ecdsa_public_key(secp, network);

        Address::p2wpkh(&ecdsa_pubkey, None, network.address_params())
    }
}
