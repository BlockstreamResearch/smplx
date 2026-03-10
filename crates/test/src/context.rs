use std::path::PathBuf;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplex_regtest::Regtest;
use simplex_regtest::client::RegtestClient;

use simplex_sdk::provider::{EsploraProvider, ProviderTrait, SimplexProvider, SimplicityNetwork};
use simplex_sdk::signer::Signer;

use crate::config::TestConfig;
use crate::error::TestError;

#[allow(dead_code)]
pub struct TestContext {
    _client: Option<RegtestClient>,
    config: TestConfig,
    signer: Signer,
}

impl TestContext {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;

        let (signer, client) = Self::setup(&config)?;

        Ok(Self {
            _client: client,
            config: config,
            signer: signer,
        })
    }

    pub fn get_provider(&self) -> &Box<dyn ProviderTrait> {
        &self.signer.get_provider()
    }

    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }

    pub fn get_network(&self) -> &SimplicityNetwork {
        &self.signer.get_provider().get_network()
    }

    pub fn get_signer(&self) -> &Signer {
        &self.signer
    }

    fn setup(config: &TestConfig) -> Result<(Signer, Option<RegtestClient>), TestError> {
        let client: Option<RegtestClient>;
        let signer: Signer;

        match config.esplora.clone() {
            Some(esplora) => match config.rpc.clone() {
                Some(rpc) => {
                    // custom regtest case
                    let auth = Auth::UserPass(rpc.username, rpc.password);
                    let provider = Box::new(SimplexProvider::new(
                        esplora.url,
                        rpc.url,
                        auth,
                        SimplicityNetwork::default_regtest(),
                    )?);

                    signer = Signer::new(config.mnemonic.as_str(), provider)?;
                    client = None;
                }
                None => {
                    // external esplora network
                    let network = match esplora.network.as_str() {
                        "Liquid" => SimplicityNetwork::Liquid,
                        "LiquidTestnet" => SimplicityNetwork::LiquidTestnet,
                        _ => panic!("Impossible branch reached, please report a bug"),
                    };
                    let provider = Box::new(EsploraProvider::new(esplora.url, network));

                    signer = Signer::new(config.mnemonic.as_str(), provider)?;
                    client = None;
                }
            },
            None => {
                // simplex inner network
                let (regtest_client, regtest_signer) = Regtest::new(config.to_regtest_config())?;

                client = Some(regtest_client);
                signer = regtest_signer;
            }
        }

        Ok((signer, client))
    }
}
