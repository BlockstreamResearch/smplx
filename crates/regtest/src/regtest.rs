use std::time::Duration;

use simplex_sdk::provider::ElementsRpc;
use simplex_sdk::provider::SimplexProvider;
use simplex_sdk::provider::SimplicityNetwork;
use simplex_sdk::signer::Signer;

use super::client::RegtestClient;
use super::RegtestConfig;
use super::error::RegtestError;

pub struct Regtest {}

impl Regtest {
    pub fn new(config: RegtestConfig) -> Result<(RegtestClient, Signer), RegtestError> {
        let client = RegtestClient::new();

        let provider = Box::new(SimplexProvider::new(
            client.esplora_url(),
            client.rpc_url(),
            client.auth(),
            SimplicityNetwork::default_regtest(),
        )?);

        let signer = Signer::new(config.mnemonic.as_str(), provider)?;

        Self::prepare_signer(&client, &signer)?;

        Ok((client, signer))
    }

    fn prepare_signer(client: &RegtestClient, signer: &Signer) -> Result<(), RegtestError> {
        let rpc_provider = ElementsRpc::new(client.rpc_url(), client.auth())?;

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

        Ok(())
    }
}
