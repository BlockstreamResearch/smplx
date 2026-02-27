mod config;
mod rpc_provider;

use crate::TestError;
pub use config::*;
use electrsd::bitcoind::bitcoincore_rpc::Auth;
pub use rpc_provider::*;
use std::io;
use std::path::PathBuf;

pub struct TestContext {
    config: ElementsDConf,
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
                let elementsd_path = ElementsDConf::obtain_default_elementsd_path();
                let rpc = TestRpcProvider::init(ConfigOption::DefaultRegtest, &elementsd_path)?;
                TestContext {
                    config: ElementsDConf {
                        elemendsd_path: elementsd_path,
                        rpc_creds: RpcCreds::None,
                    },
                    rpc,
                }
            }
            Self::FromConfigPath(path) => {
                let config: ElementsDConf = ElementsDConf::from_file(&path)?;
                match &config.rpc_creds {
                    RpcCreds::Auth {
                        url,
                        username,
                        password,
                    } => {
                        let rpc = TestRpcProvider::init(
                            ConfigOption::CustomRpcUrlRegtest {
                                url: url.clone(),
                                auth: Auth::UserPass(username.clone(), password.clone()),
                            },
                            &config.elemendsd_path,
                        )?;
                        TestContext { config, rpc }
                    }
                    RpcCreds::None => {
                        let rpc = TestRpcProvider::init(ConfigOption::DefaultRegtest, &config.elemendsd_path)?;
                        TestContext { config, rpc }
                    }
                }
            }
        };
        Ok(context)
    }
}

impl TestContext {
    pub fn get_config(&self) -> &ElementsDConf {
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
