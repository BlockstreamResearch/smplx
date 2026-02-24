mod config;
mod rpc_provider;

use crate::TestError;
pub use config::*;
use electrsd::bitcoind::bitcoincore_rpc::Auth;
pub use rpc_provider::*;
use std::path::PathBuf;

pub struct TestContext {
    config: TestConfig,
    rpc: TestRpcProvider,
}

pub enum TestContextBuilder {
    Default,
    FromConfigPath(PathBuf),
}

impl TestContextBuilder {
    pub fn build(self) -> Result<TestContext, TestError> {
        let context = match self {
            Self::Default => {
                let rpc = TestRpcProvider::init(ConfigOption::DefaultRegtest)?;
                TestContext {
                    config: TestConfig::default(),
                    rpc,
                }
            }
            Self::FromConfigPath(path) => {
                let config: TestConfig = TestConfig::from_file(&path)?;
                match &config.rpc_creds {
                    RpcCreds::Auth {
                        url,
                        username,
                        password,
                    } => {
                        let rpc = TestRpcProvider::init(ConfigOption::CustomRpcUrlRegtest {
                            url: url.clone(),
                            auth: Auth::UserPass(username.clone(), password.clone()),
                        })?;
                        TestContext { config, rpc }
                    }
                    RpcCreds::None => {
                        let rpc = TestRpcProvider::init(ConfigOption::DefaultRegtest)?;
                        TestContext { config, rpc }
                    }
                }
            }
        };
        Ok(context)
    }
}

impl TestContext {
    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }

    pub fn get_rpc_provider(&self) -> &TestRpcProvider {
        &self.rpc
    }

    pub fn default_rpc_setup(&self) -> Result<(), TestError> {
        self.rpc.generate_blocks(1)?;
        self.rpc.rescanblockchain(None, None)?;
        self.rpc.sweep_initialfreecoins()?;
        self.rpc.generate_blocks(100)?;
        Ok(())
    }
}
