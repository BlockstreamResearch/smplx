/// Dummy signature, which is used for fee estimation
pub const DUMMY_SIGNATURE: [u8; 64] = [1; 64];

/// Minimal acceptable fee for nodes to relay a transaction
pub const MIN_FEE: u64 = 10;

/// The smallest explicit output Elements relays by default for any standard script.
/// 14 sats for P2WPKH, 15 for P2TR, 22 for P2PKH and P2SH.
pub const DUST_THRESHOLD: u64 = 22;

/// Policy `AssetId` (hex, BE) for Liquid mainnet.
pub const LIQUID_POLICY_ASSET_STR: &str = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

/// Policy `AssetId` (hex, BE) for Liquid testnet.
pub const LIQUID_TESTNET_POLICY_ASSET_STR: &str = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

/// Policy `AssetId` (hex, BE) for Elements regtest.
pub const LIQUID_DEFAULT_REGTEST_ASSET_STR: &str = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";
