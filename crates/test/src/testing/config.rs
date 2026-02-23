use electrsd::bitcoind::bitcoincore_rpc::jsonrpc::serde::{Deserialize, Serialize};

pub const TEST_ENV_NAME: &str = "SIMPLEX_TEST_ENV";

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct TestConfig {
    pub rpc_creds: RpcCreds,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub enum RpcCreds {
    Auth {
        rpc_username: String,
        rpc_password: String,
    },
    #[default]
    None,
}
