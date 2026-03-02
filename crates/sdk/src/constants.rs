pub const PUBLIC_SECRET_BLINDER_KEY: [u8; 32] = [1; 32];
pub const DUMMY_SIGNATURE: [u8; 64] = [1; 64];

pub const MIN_FEE: u64 = 10;

/// Policy asset id (hex, BE) for Liquid mainnet.
pub const LIQUID_POLICY_ASSET_STR: &str = "6f0279e9ed041c3d710a9f57d0c02928416460c4b722ae3457a11eec381c526d";

/// Policy asset id (hex, BE) for Liquid testnet.
pub const LIQUID_TESTNET_POLICY_ASSET_STR: &str = "144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49";

/// Policy asset id (hex, BE) for Elements regtest.
pub const LIQUID_DEFAULT_REGTEST_ASSET_STR: &str = "5ac9f65c0efcc4775e0baec4ec03abdde22473cd3cf33c0419ca290e0751b225";

/// Example test asset id (hex, BE) on Liquid testnet.
pub const LIQUID_TESTNET_TEST_ASSET_ID_STR: &str = "38fca2d939696061a8f76d4e6b5eecd54e3b4221c846f24a6b279e79952850a5";
