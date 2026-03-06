use std::path::PathBuf;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplex_regtest::TestClient;
use simplex_sdk::provider::{EsploraProvider, ProviderTrait, SimplexProvider};

use crate::config::TestConfig;
use crate::error::TestError;

pub struct TestContext {
    config: TestConfig,
    provider: Box<dyn ProviderTrait>,
}

impl TestContext {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;
        let provider: Box<dyn ProviderTrait>;

        match config.esplora.clone() {
            Some(esplora) => match config.rpc.clone() {
                Some(rpc) => {
                    let auth = Auth::UserPass(rpc.username, rpc.password);

                    provider = Box::new(SimplexProvider::new(esplora, rpc.url, auth)?);
                }
                None => {
                    provider = Box::new(EsploraProvider::new(esplora));
                }
            },
            None => {
                let client = TestClient::new();

                provider = Box::new(SimplexProvider::new(
                    client.esplora_url(),
                    client.rpc_url(),
                    client.auth(),
                )?);
            }
        }

        // TODO: setup signer

        Ok(Self {
            config: config,
            provider: provider,
        })
    }

    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }

    pub fn get_provider(&self) -> &Box<dyn ProviderTrait> {
        &self.provider
    }

    // TODO: how to do this better?

    // pub fn default_rpc_setup(&self) -> Result<(), TestError> {
    //     self.rpc.generate_blocks(1)?;
    //     self.rpc.rescanblockchain(None, None)?;
    //     self.rpc.sweep_initialfreecoins()?;
    //     self.rpc.generate_blocks(100)?;
    //     Ok(())
    // }
}
