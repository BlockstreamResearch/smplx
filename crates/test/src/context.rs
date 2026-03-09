use std::path::PathBuf;
use std::time::Duration;

use electrsd::bitcoind::bitcoincore_rpc::Auth;

use simplex_regtest::TestClient;
use simplex_sdk::provider::ElementsRpc;
use simplex_sdk::provider::{EsploraProvider, ProviderTrait, SimplexProvider, SimplicityNetwork};
use simplex_sdk::signer::Signer;

use crate::config::TestConfig;
use crate::error::TestError;

#[allow(dead_code)]
pub struct TestContext {
    _client: Option<TestClient>,
    config: TestConfig,
    signer: Signer,
}

impl TestContext {
    pub fn new(config_path: PathBuf) -> Result<Self, TestError> {
        let config = TestConfig::from_file(&config_path)?;

        let (provider, client) = Self::setup_provider(&config)?;
        let signer = Self::setup_signer(provider, &client, &config.mnemonic)?;

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

    fn setup_provider(config: &TestConfig) -> Result<(Box<dyn ProviderTrait>, Option<TestClient>), TestError> {
        let provider: Box<dyn ProviderTrait>;
        let client: Option<TestClient>;

        match config.esplora.clone() {
            Some(esplora) => match config.rpc.clone() {
                Some(rpc) => {
                    // custom regtest case
                    let auth = Auth::UserPass(rpc.username, rpc.password);

                    provider = Box::new(SimplexProvider::new(
                        esplora.url,
                        rpc.url,
                        auth,
                        SimplicityNetwork::default_regtest(),
                    )?);
                    client = None;
                }
                None => {
                    // external esplora network
                    let network = match esplora.network.as_str() {
                        "Liquid" => SimplicityNetwork::Liquid,
                        "LiquidTestnet" => SimplicityNetwork::LiquidTestnet,
                        _ => panic!("Impossible branch reached, please report a bug"),
                    };

                    provider = Box::new(EsploraProvider::new(esplora.url, network));
                    client = None;
                }
            },
            None => {
                // simplex inner network
                let client_inner = TestClient::new();

                provider = Box::new(SimplexProvider::new(
                    client_inner.esplora_url(),
                    client_inner.rpc_url(),
                    client_inner.auth(),
                    SimplicityNetwork::default_regtest(),
                )?);

                // need to save the client so that rust doesn't kill it
                client = Some(client_inner);
            }
        }

        Ok((provider, client))
    }

    fn setup_signer(
        provider: Box<dyn ProviderTrait>,
        client: &Option<TestClient>,
        mnemonic: &String,
    ) -> Result<Signer, TestError> {
        let signer = Signer::new(mnemonic, provider)?;

        match client {
            // if client exists, we are using inner simplex network
            Some(client_inner) => {
                let rpc_provider = ElementsRpc::new(client_inner.rpc_url(), client_inner.auth())?;

                rpc_provider.generate_blocks(1)?;
                rpc_provider.rescanblockchain(None, None)?;
                rpc_provider.sweep_initialfreecoins()?;
                rpc_provider.generate_blocks(100)?;

                // 20 million BTC
                rpc_provider.sendtoaddress(&signer.get_wpkh_address()?, 20_000_000 * u64::pow(10, 8), None)?;

                // wait for electrs to index
                let mut attempts = 0;

                loop {
                    if !(signer.get_wpkh_utxos()?).is_empty() {
                        break;
                    }

                    attempts += 1;

                    if attempts > 100 {
                        panic!("Electrs failed to index the sweep after 10 seconds");
                    }

                    std::thread::sleep(Duration::from_millis(100));
                }
            }
            None => {}
        };

        Ok(signer)
    }
}
