use std::path::PathBuf;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use smplx_regtest::Regtest;
use smplx_regtest::client::RegtestClient;

use smplx_sdk::provider::{EsploraProvider, ProviderInfo, ProviderTrait, SimplexProvider, SimplicityNetwork};
use smplx_sdk::signer::Signer;
use smplx_sdk::utils::random_mnemonic;

use crate::config::TestConfig;
use crate::error::TestError;

#[allow(dead_code)]
pub struct TestContext {
    _client: Option<RegtestClient>,
    // since providers can't be cloned, we need this variable to create new signers
    _provider_info: ProviderInfo,
    config: TestConfig,
    signer: Signer,
}

impl TestContext {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;

        let (signer, provider_info, client) = Self::setup(&config)?;

        Ok(Self {
            _client: client,
            _provider_info: provider_info,
            config,
            signer,
        })
    }

    pub fn create_signer(&self, mnemonic: &str) -> Signer {
        let provider: Box<dyn ProviderTrait> = if self._provider_info.elements_url.is_some() {
            // local regtest or external regtest
            Box::new(SimplexProvider::new(
                self._provider_info.esplora_url.clone(),
                self._provider_info.elements_url.clone().unwrap(),
                self._provider_info.auth.clone().unwrap(),
                *self.get_network(),
            ))
        } else {
            // external esplora
            Box::new(EsploraProvider::new(
                self._provider_info.esplora_url.clone(),
                *self.get_network(),
            ))
        };

        Signer::new(mnemonic, provider)
    }

    pub fn random_signer(&self) -> Signer {
        self.create_signer(random_mnemonic().as_str())
    }

    pub fn get_default_signer(&self) -> &Signer {
        &self.signer
    }

    pub fn get_default_provider(&self) -> &dyn ProviderTrait {
        self.signer.get_provider()
    }

    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }

    pub fn get_network(&self) -> &SimplicityNetwork {
        self.signer.get_provider().get_network()
    }

    fn setup(config: &TestConfig) -> Result<(Signer, ProviderInfo, Option<RegtestClient>), TestError> {
        let client: Option<RegtestClient>;
        let provider_info: ProviderInfo;
        let signer: Signer;

        match config.esplora.clone() {
            Some(esplora) => match config.rpc.clone() {
                Some(rpc) => {
                    // custom regtest case
                    let auth = Auth::UserPass(rpc.username, rpc.password);
                    let provider = Box::new(SimplexProvider::new(
                        esplora.url.clone(),
                        rpc.url.clone(),
                        auth.clone(),
                        SimplicityNetwork::default_regtest(),
                    ));

                    provider_info = ProviderInfo {
                        esplora_url: esplora.url,
                        elements_url: Some(rpc.url),
                        auth: Some(auth),
                    };
                    signer = Signer::new(config.mnemonic.as_str(), provider);
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
                    let provider = Box::new(EsploraProvider::new(esplora.url.clone(), network));

                    provider_info = ProviderInfo {
                        esplora_url: esplora.url,
                        elements_url: None,
                        auth: None,
                    };
                    signer = Signer::new(config.mnemonic.as_str(), provider);
                    client = None;
                }
            },
            None => {
                // simplex inner network
                let (regtest_client, regtest_signer) = Regtest::from_config(config.to_regtest_config())?;

                provider_info = ProviderInfo {
                    esplora_url: regtest_client.esplora_url(),
                    elements_url: Some(regtest_client.rpc_url()),
                    auth: Some(regtest_client.auth()),
                };
                signer = regtest_signer;
                client = Some(regtest_client);
            }
        }

        Ok((signer, provider_info, client))
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
