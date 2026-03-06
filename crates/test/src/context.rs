use std::path::PathBuf;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplex_regtest::TestClient;
use simplex_sdk::provider::{EsploraProvider, ProviderTrait, SimplexProvider};

use crate::config::TestConfig;
use crate::error::TestError;

pub struct TestContext {
    client: Option<TestClient>,
    config: TestConfig,
    provider: Box<dyn ProviderTrait>,
}

impl TestContext {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;
        let provider: Box<dyn ProviderTrait>;
        let mut client: Option<TestClient> = None;

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
                let client_inner = TestClient::new();

                provider = Box::new(SimplexProvider::new(
                    client_inner.esplora_url(),
                    client_inner.rpc_url(),
                    client_inner.auth(),
                )?);

                client = Some(client_inner);

                // TODO
                // signer = Signer::new(config.seed);

                // provider.rpc.generate_blocks(1)?;
                // provider.rpc.rescanblockchain(None, None)?;
                // provider.rpc.sweep_initialfreecoins()?;
                // provider.rpc.generate_blocks(100)?;

                // provider.rpc.sendtoaddress(signer)
            }
        }

        Ok(Self {
            client: client,
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
}
