use std::time::Duration;

use smplx_sdk::provider::ElementsRpc;
use smplx_sdk::provider::SimplexProvider;
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::utils::btc2sat;

use super::RegtestConfig;
use super::client::RegtestClient;
use super::error::RegtestError;

pub struct Regtest {}

impl Regtest {
    /// Initializes a Regtest environment and returns a configured client and funded signer.
    ///
    /// This method establishes a connection to the backend, sets up the provider,
    /// and prepares the `Signer` by generating initial blocks and sweeping funds based on the configuration.
    ///
    /// # Errors
    /// Returns a `RegtestError` if node initialization, block generation, or RPC calls fail.
    ///
    /// # Panics
    /// Panics if the background indexer (`electrs`) fails to index the unspent outputs within the timeout window (10 seconds).
    pub fn from_config(config: &RegtestConfig) -> Result<(RegtestClient, Signer), RegtestError> {
        let client = RegtestClient::new(config);

        let provider = Box::new(SimplexProvider::new(
            client.esplora_url(),
            client.rpc_url(),
            client.auth(),
            SimplicityNetwork::default_regtest(),
        ));

        let signer = Signer::new(config.mnemonic.as_str(), provider);

        Self::prepare_signer(&client, &signer, config.bitcoins)?;

        Ok((client, signer))
    }

    fn prepare_signer(client: &RegtestClient, signer: &Signer, bitcoins: u64) -> Result<(), RegtestError> {
        let rpc_provider = ElementsRpc::new(client.rpc_url(), client.auth())?;

        rpc_provider.generate_blocks(1)?;
        rpc_provider.rescan_blockchain(None, None)?;
        rpc_provider.sweep_initialfreecoins()?;
        rpc_provider.generate_blocks(100)?;

        rpc_provider.send_to_address(&signer.get_address(), btc2sat(bitcoins), None)?;
        rpc_provider.generate_blocks(1)?;

        // wait for electrs to index
        let mut attempts = 0;

        loop {
            if !(signer.get_utxos()?).is_empty() {
                break;
            }

            attempts += 1;

            assert!(attempts <= 100, "Electrs failed to index the sweep after 10 seconds");

            std::thread::sleep(Duration::from_millis(100));
        }

        Ok(())
    }
}
