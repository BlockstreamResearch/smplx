use crate::error::ExplorerError;
use crate::Network;
use corepc_client::client_sync::{v23::Client, Auth};

mod types;

pub struct ElementsRpcClient {
    inner: Client,
    #[allow(unused)]
    network: Network,
    #[allow(unused)]
    auth: Auth,
    #[allow(unused)]
    url: String,
}

impl ElementsRpcClient {
    pub fn new(network: Network, url: &str, auth: Auth) -> Result<Self, ExplorerError> {
        let inner = Client::new_with_auth(url, auth.clone())?;
        inner.ping()?;
        Ok(Self {
            inner,
            network,
            auth,
            url: url.to_string(),
        })
    }

    pub fn new_from_credentials(network: Network, url: &str, user: &str, pass: &str) -> Result<Self, ExplorerError> {
        let auth = Auth::UserPass(user.to_string(), pass.to_string());
        Self::new(network, url, auth)
    }

    pub fn height(&self) -> Result<u64, ExplorerError> {
        const METHOD: &str = "getblockcount";

        self.inner
            .call::<serde_json::Value>(METHOD, &[])?
            .as_u64()
            .ok_or_else(|| ExplorerError::ElementsRpcUnexpectedReturn(METHOD.into()))
    }

    pub fn blockchain_info(&self) -> Result<corepc_types::v23::GetBlockchainInfo, ExplorerError> {
        const METHOD: &str = "getblockchaininfo";

        Ok(self.inner.call::<corepc_types::v23::GetBlockchainInfo>(METHOD, &[])?)
    }

    pub fn inner(&self) -> &Client{
        &self.inner
    }
}
