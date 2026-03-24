use std::path::PathBuf;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use smplx_regtest::Regtest;
use smplx_regtest::client::RegtestClient;

use smplx_sdk::provider::{EsploraProvider, ProviderTrait, SimplexProvider, SimplicityNetwork};
use smplx_sdk::signer::Signer;

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
            config,
            signer,
        })
    }

    pub fn get_provider(&self) -> &dyn ProviderTrait {
        self.signer.get_provider()
    }

    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }

    pub fn get_network(&self) -> &SimplicityNetwork {
        self.signer.get_provider().get_network()
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
                        "ElementsRegtest" => SimplicityNetwork::default_regtest(),
                        other => return Err(TestError::BadNetworkName(other.to_string())),
                    };
                    let provider = Box::new(EsploraProvider::new(esplora.url, network));

                    signer = Signer::new(config.mnemonic.as_str(), provider)?;
                    client = None;
                }
            },
            None => {
                // simplex inner network
                let (regtest_client, regtest_signer) = Regtest::from_config(config.to_regtest_config())?;

                client = Some(regtest_client);
                signer = regtest_signer;
            }
        }

        Ok((signer, client))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn invalid_network_returns_error() {
        let config = r#"
            mnemonic = "exist carry drive collect lend cereal occur much tiger just involve mean"
            bitcoins = 10000

            [esplora]
            url = "http://localhost:3000"
            network = "InvalidNetwork"
        "#;

        let path = std::env::temp_dir().join("smplx_test_invalid_network.toml");
        fs::write(&path, config).unwrap();

        let result = TestContext::new(path);
        let Err(e) = result else {
            panic!("expected BadNetworkName error")
        };
        assert!(
            matches!(e, TestError::BadNetworkName(ref s) if s == "InvalidNetwork"),
            "expected BadNetworkName, got: {e}"
        );
    }
}
