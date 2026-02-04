//! Common Liquid network constants and helpers.
//!
//! Exposes policy asset identifiers and the Liquid testnet genesis hash.
//!
//! These are used throughout the CLI and examples to ensure consistent
//! parameters when constructing Elements transactions.

use simplicityhl::simplicity::elements;
use simplicityhl::simplicity::hashes::{Hash, sha256};
use std::str::FromStr;

pub const PUBLIC_SECRET_BLINDER_KEY: [u8; 32] = [1; 32];

/// `PLACEHOLDER_ISSUANCE_VALUE` constant is used for issuance and reissuance tokens during the blinding process.
///
/// During blinding, the PSET collects surjection proof inputs from all outputs with matching asset IDs.
/// For issuance tokens, only the `asset_id` from `TxOutSectet` is copied into surjection proofs while the `value` is set to `0`,
/// as the exact value inserted for the issuance token is irrelevant to the proof computation.
/// This is because issuance and reissuance tokens surjection proofs only care about the `asset_id` or `token_id`, not the token value.
///
/// See: `<https://github.com/ElementsProject/rust-elements/blob/2d94f5dcb5e4a81838681c71ed8a49b337feecd2/src/pset/mod.rs#L438>`
pub const PLACEHOLDER_ISSUANCE_VALUE: u64 = 0;

/// Policy asset id (hex, BE) for Liquid mainnet.
pub const LIQUID_POLICY_ASSET_STR: &str =
    "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

/// Policy asset id (hex, BE) for Liquid testnet.
pub const LIQUID_TESTNET_POLICY_ASSET_STR: &str =
    "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

/// Policy asset id (hex, BE) for Elements regtest.
pub const LIQUID_DEFAULT_REGTEST_ASSET_STR: &str =
    "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

/// Example test asset id (hex, BE) on Liquid testnet.
pub static LIQUID_TESTNET_TEST_ASSET_ID_STR: &str =
    "38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5";

/// LBTC asset id for Liquid testnet.
pub static LIQUID_TESTNET_BITCOIN_ASSET: std::sync::LazyLock<elements::AssetId> =
    std::sync::LazyLock::new(|| {
        elements::AssetId::from_inner(sha256::Midstate([
            0x49, 0x9a, 0x81, 0x85, 0x45, 0xf6, 0xba, 0xe3, 0x9f, 0xc0, 0x3b, 0x63, 0x7f, 0x2a,
            0x4e, 0x1e, 0x64, 0xe5, 0x90, 0xca, 0xc1, 0xbc, 0x3a, 0x6f, 0x6d, 0x71, 0xaa, 0x44,
            0x43, 0x65, 0x4c, 0x14,
        ]))
    });

/// Genesis block hash for Liquid mainnet.
pub static LIQUID_MAINNET_GENESIS: std::sync::LazyLock<elements::BlockHash> =
    std::sync::LazyLock::new(|| {
        elements::BlockHash::from_byte_array([
            0x03, 0x60, 0x20, 0x8a, 0x88, 0x96, 0x92, 0x37, 0x2c, 0x8d, 0x68, 0xb0, 0x84, 0xa6,
            0x2e, 0xfd, 0xf6, 0x0e, 0xa1, 0xa3, 0x59, 0xa0, 0x4c, 0x94, 0xb2, 0x0d, 0x22, 0x36,
            0x58, 0x27, 0x66, 0x14,
        ])
    });

/// Genesis block hash for Liquid testnet.
pub static LIQUID_TESTNET_GENESIS: std::sync::LazyLock<elements::BlockHash> =
    std::sync::LazyLock::new(|| {
        elements::BlockHash::from_byte_array([
            0xc1, 0xb1, 0x6a, 0xe2, 0x4f, 0x24, 0x23, 0xae, 0xa2, 0xea, 0x34, 0x55, 0x22, 0x92,
            0x79, 0x3b, 0x5b, 0x5e, 0x82, 0x99, 0x9a, 0x1e, 0xed, 0x81, 0xd5, 0x6a, 0xee, 0x52,
            0x8e, 0xda, 0x71, 0xa7,
        ])
    });

/// Genesis block hash for Liquid regtest.
pub static LIQUID_REGTEST_GENESIS: std::sync::LazyLock<elements::BlockHash> =
    std::sync::LazyLock::new(|| {
        elements::BlockHash::from_byte_array([
            0x21, 0xca, 0xb1, 0xe5, 0xda, 0x47, 0x18, 0xea, 0x14, 0x0d, 0x97, 0x16, 0x93, 0x17,
            0x02, 0x42, 0x2f, 0x0e, 0x6a, 0xd9, 0x15, 0xc8, 0xd9, 0xb5, 0x83, 0xca, 0xc2, 0x70,
            0x6b, 0x2a, 0x90, 0x00,
        ])
    });

/// The network of the elements blockchain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimplicityNetwork {
    // Liquid mainnet policy asset
    Liquid,
    // Liquid testnet policy asset
    LiquidTestnet,
    /// Liquid regtest with a custom policy asset.
    ElementsRegtest {
        /// The policy asset to use for this regtest network.
        /// You can use the default one using [`SimplicityNetwork::default_regtest()`].
        policy_asset: elements::AssetId,
    },
}

impl SimplicityNetwork {
    /// Return the default policy asset for regtest network.
    ///
    /// # Panics
    ///
    /// Doesn't panic as constants are defined correctly.
    #[must_use]
    pub fn default_regtest() -> Self {
        let policy_asset = elements::AssetId::from_str(LIQUID_DEFAULT_REGTEST_ASSET_STR).unwrap();
        Self::ElementsRegtest { policy_asset }
    }

    /// Return the policy asset for specific network.
    ///
    /// # Panics
    ///
    /// Doesn't panic as constants are defined correctly.
    #[must_use]
    pub fn policy_asset(&self) -> elements::AssetId {
        match self {
            Self::Liquid => elements::AssetId::from_str(LIQUID_POLICY_ASSET_STR).unwrap(),
            Self::LiquidTestnet => {
                elements::AssetId::from_str(LIQUID_TESTNET_POLICY_ASSET_STR).unwrap()
            }
            Self::ElementsRegtest { policy_asset } => *policy_asset,
        }
    }

    /// Return the genesis block hash for this network.
    #[must_use]
    pub fn genesis_block_hash(&self) -> elements::BlockHash {
        match self {
            Self::Liquid => *LIQUID_MAINNET_GENESIS,
            Self::LiquidTestnet => *LIQUID_TESTNET_GENESIS,
            Self::ElementsRegtest { .. } => *LIQUID_REGTEST_GENESIS,
        }
    }

    /// Return the address parameters for this network to generate addresses compatible for this network.
    #[must_use]
    pub const fn address_params(&self) -> &'static elements::AddressParams {
        match self {
            Self::Liquid => &elements::AddressParams::LIQUID,
            Self::LiquidTestnet => &elements::AddressParams::LIQUID_TESTNET,
            Self::ElementsRegtest { .. } => &elements::AddressParams::ELEMENTS,
        }
    }
}
