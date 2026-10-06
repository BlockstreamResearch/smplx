use std::path::PathBuf;
use std::str::FromStr;

use electrsd::bitcoind::bitcoincore_rpc::Auth;
use proptest::prelude::Strategy;
use proptest::test_runner::{Config as ProptestConfig, FileFailurePersistence};
use simplicityhl::{Arguments, WitnessValues};

use smplx_regtest::Regtest;
use smplx_regtest::client::RegtestClient;
use smplx_sdk::global::GlobalConfig;
use smplx_sdk::program::{ProgramFactory, RandomArguments, RandomWitness};
use smplx_sdk::provider::{
    ElementsRpc, EsploraProvider, ProviderInfo, ProviderTrait, SimplexProvider, SimplicityNetwork,
};
use smplx_sdk::signer::Signer;
use smplx_sdk::utils::random_mnemonic;

use crate::config::TestConfig;
use crate::error::{FuzzError, TestError};
use crate::network_utils::NetworkUtils;

pub use crate::context::sealed::ContextMode;
use crate::fuzz::engine::FuzzContext;
use crate::fuzz::transaction::FuzzTransaction;
use crate::fuzz::{FuzzableProgram, SimplexFuzzEngine};

#[allow(unreachable_pub)]
mod sealed {
    pub trait ContextMode {}
}

pub struct InitMode;

pub struct RegularMode {
    signer: Signer,
    _provider_info: ProviderInfo,
    _client: Option<RegtestClient>,
}

pub struct FuzzMode {
    proptest_config: proptest::test_runner::Config,
    signer: Option<Signer>,
    network: Option<SimplicityNetwork>,
}

impl ContextMode for InitMode {}
impl ContextMode for RegularMode {}
impl ContextMode for FuzzMode {}

pub struct TestContext<Mode: ContextMode = RegularMode> {
    config: TestConfig,
    pub(crate) mode: Mode,
}

impl TestContext<InitMode> {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;
        Self::from_config(config)
    }

    pub fn from_config(config: TestConfig) -> Result<Self, TestError> {
        // error is ignored because we assume that all tests use the same verbosity
        let _ = GlobalConfig::set_global_config(config.verbosity);

        Ok(Self {
            config,
            mode: InitMode {},
        })
    }

    pub fn regular(self) -> Result<TestContext<RegularMode>, TestError> {
        let mode = RegularMode::setup(self.get_config())?;

        Ok(TestContext::<RegularMode> {
            config: self.config,
            mode,
        })
    }

    pub fn fuzz(self, test_name: &'static str, source_file: &'static str) -> Result<TestContext<FuzzMode>, TestError> {
        let mode = FuzzMode::setup(self.get_config(), test_name, source_file)?;

        Ok(TestContext::<FuzzMode> {
            config: self.config,
            mode,
        })
    }
}

impl RegularMode {
    fn setup(config: &TestConfig) -> Result<RegularMode, TestError> {
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
                    let network = SimplicityNetwork::from_str(esplora.network.as_str())?;
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
                let (regtest_client, regtest_signer) = Regtest::from_config(&config.to_regtest_config())?;

                provider_info = ProviderInfo {
                    esplora_url: regtest_client.esplora_url(),
                    elements_url: Some(regtest_client.rpc_url()),
                    auth: Some(regtest_client.auth()),
                };
                signer = regtest_signer;
                client = Some(regtest_client);
            }
        }

        Ok(RegularMode {
            signer,
            _provider_info: provider_info,
            _client: client,
        })
    }
}

impl TestContext<RegularMode> {
    pub fn get_network_utils(&self) -> NetworkUtils {
        assert!(
            self.mode._client.is_some(),
            "Network utils only available in Regtest network"
        );

        let regtest_rpc = ElementsRpc::new(
            self.mode._provider_info.elements_url.clone().unwrap(),
            self.mode._provider_info.auth.clone().unwrap(),
        )
        .expect("Failed to create rpc client for network utils");

        let network = self.get_network();
        let esplora = EsploraProvider::new(self.mode._provider_info.esplora_url.clone(), *network);

        NetworkUtils::new(regtest_rpc, esplora)
    }

    pub fn create_signer(&self, mnemonic: &str) -> Signer {
        let provider: Box<dyn ProviderTrait> = if self.mode._provider_info.elements_url.is_some() {
            // local regtest or external regtest
            Box::new(SimplexProvider::new(
                self.mode._provider_info.esplora_url.clone(),
                self.mode._provider_info.elements_url.clone().unwrap(),
                self.mode._provider_info.auth.clone().unwrap(),
                *self.get_network(),
            ))
        } else {
            // external esplora
            Box::new(EsploraProvider::new(
                self.mode._provider_info.esplora_url.clone(),
                *self.get_network(),
            ))
        };

        Signer::new(mnemonic, provider)
    }

    pub fn random_signer(&self) -> Signer {
        self.create_signer(random_mnemonic().as_str())
    }

    pub fn get_default_signer(&self) -> &Signer {
        &self.mode.signer
    }

    /// # Panics
    /// Panics when the signer was built without a provider, which a test context never is.
    pub fn get_default_provider(&self) -> &dyn ProviderTrait {
        self.mode
            .signer
            .get_provider()
            .expect("a test context always has a provider")
    }

    /// # Panics
    /// Panics when the signer was built without a provider, which a test context never is.
    pub fn get_network(&self) -> &SimplicityNetwork {
        self.get_default_provider().get_network()
    }
}

impl FuzzMode {
    fn setup(config: &TestConfig, test_name: &'static str, source_file: &'static str) -> Result<FuzzMode, TestError> {
        const FUZZ_FAILURES_FOLDER_NAME: &str = "fuzz-failures";

        let mut proptest_config = ProptestConfig {
            verbose: config.verbosity as u32,
            test_name: Some(test_name),
            fork: false,
            max_shrink_iters: 0,
            source_file: Some(source_file),
            ..ProptestConfig::default()
        };

        if let Some(persistence) = proptest_config.failure_persistence.as_mut() {
            *persistence = Box::new(FileFailurePersistence::SourceParallel(FUZZ_FAILURES_FOLDER_NAME));
        }

        let (mut network, mut signer) = (None, None);

        if let Some(fuzz_config) = config.fuzz.as_ref() {
            if let Some(cases) = fuzz_config.cases {
                proptest_config.cases = cases;
            }

            if let Some(max_global_rejects) = fuzz_config.max_global_rejects {
                proptest_config.max_global_rejects = max_global_rejects;
            }

            if let Some(max_local_rejects) = fuzz_config.max_local_rejects {
                proptest_config.max_local_rejects = max_local_rejects;
            }

            if let Some(n) = fuzz_config.network.as_ref() {
                let _ = network.insert(SimplicityNetwork::from_str(n.as_str())?);
                let _ = signer.insert(Signer::from_mnemonic(&config.mnemonic, *network.as_ref().unwrap()));
            }
        }

        Ok(FuzzMode {
            proptest_config,
            signer,
            network,
        })
    }

    fn create_signer(&self, mnemonic: &str) -> Option<Signer> {
        self.network.map(|n| Signer::from_mnemonic(mnemonic, n))
    }
}

impl TestContext<FuzzMode> {
    /// Creates a random signer from a random mnemonic
    pub fn random_signer(&self) -> Option<Signer> {
        self.mode.create_signer(random_mnemonic().as_str())
    }

    /// Replaces the fuzz signer and synchronizes the fuzz network with it.
    pub fn set_custom_signer(&mut self, signer: Signer) {
        let _ = self.mode.network.insert(*signer.get_network());
        let _ = self.mode.signer.insert(signer);
    }

    /// Sets the fuzz network when it is compatible with the configured signer.
    ///
    /// # Errors
    /// Returns [`FuzzError::SignerNetworkMismatch`] when the signer uses a different network.
    pub fn set_network(&mut self, network: SimplicityNetwork) -> Result<(), FuzzError> {
        if let Some(signer) = self.mode.signer.as_ref() {
            let signer_network = *signer.get_network();

            if network != signer_network {
                return Err(FuzzError::SignerNetworkMismatch {
                    network: format!("{:?}", network),
                    signer_network: format!("{:?}", signer_network),
                });
            }
        }

        let _ = self.mode.network.insert(network);
        Ok(())
    }

    /// Returns internal signer which is already derived or reassigned by a user.
    pub fn get_default_signer(&self) -> &Option<Signer> {
        &self.mode.signer
    }

    /// Sets the maximum number of combined inputs that may be rejected before the test as a whole aborts.
    pub fn set_max_global_rejects(&mut self, max_global_rejects: u32) {
        self.mode.proptest_config.max_global_rejects = max_global_rejects;
    }

    /// Sets the number of successful test cases that must execute for the test as a whole to pass.
    pub fn set_cases(&mut self, cases: u32) {
        self.mode.proptest_config.cases = cases;
    }

    /// Sets the maximum number of individual inputs that may be rejected before the test as a whole aborts.
    pub fn set_max_local_rejects(&mut self, max_local_rejects: u32) {
        self.mode.proptest_config.max_local_rejects = max_local_rejects;
    }

    /// Builds a fuzz engine using the configured signer and network.
    ///
    /// Sets network to regtest, when it's absent.
    pub fn build<Program, Args, Wit>(
        self,
        strategy_storage: impl Strategy<Value = (Arguments, WitnessValues)> + 'static,
        blueprint: FuzzTransaction,
    ) -> SimplexFuzzEngine<Program, Args, Wit>
    where
        Program: FuzzableProgram<Program> + ProgramFactory<Program> + Clone + 'static,
        Args: Into<Arguments> + RandomArguments + std::fmt::Debug + Clone + 'static,
        Wit: Into<WitnessValues> + RandomWitness + std::fmt::Debug + Clone + 'static,
    {
        let network = self.mode.network.unwrap_or(SimplicityNetwork::default_regtest());

        SimplexFuzzEngine {
            runner: proptest::test_runner::TestRunner::new(self.mode.proptest_config),
            context: FuzzContext {
                signer: self.mode.signer,
                network,
            },
            strategy: strategy_storage.boxed(),
            blueprint,
            _placeholder: Default::default(),
        }
    }
}

impl<T: ContextMode> TestContext<T> {
    pub fn get_config(&self) -> &TestConfig {
        &self.config
    }
}

impl Drop for RegularMode {
    fn drop(&mut self) {
        if let Some(x) = &mut self._client {
            let _ = x.kill();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smplx_sdk::provider::NetworkConvertError;
    use std::fs;

    #[test]
    fn invalid_network_returns_error() {
        let config = r#"
            mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
            bitcoins = 10000

            [esplora]
            url = "http://localhost:3000"
            network = "InvalidNetwork"
        "#;

        let path = std::env::temp_dir().join("smplx_test_invalid_network.toml");
        fs::write(&path, config).unwrap();

        let result = TestContext::new(path.clone()).and_then(TestContext::regular);
        fs::remove_file(path).unwrap();
        let Err(e) = result else {
            panic!("expected BadNetworkName error")
        };
        assert!(
            matches!(e, TestError::BadNetworkName(NetworkConvertError::BadNetworkName(ref s)) if s == "InvalidNetwork"),
            "expected BadNetworkName, got: {e}"
        );
    }

    #[test]
    fn invalid_fuzz_network_returns_error() {
        let config = TestConfig {
            fuzz: Some(crate::config::FuzzConfig {
                network: Some("InvalidNetwork".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };

        let result = TestContext::from_config(config)
            .unwrap()
            .fuzz("invalid_fuzz_network_returns_error", file!());
        let Err(e) = result else {
            panic!("expected BadNetworkName error")
        };
        assert!(
            matches!(e, TestError::BadNetworkName(NetworkConvertError::BadNetworkName(ref s)) if s == "InvalidNetwork"),
            "expected BadNetworkName, got: {e}"
        );
    }
}
