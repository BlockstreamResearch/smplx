use reqwest::{StatusCode, Url};

#[derive(thiserror::Error, Debug)]
pub enum ExplorerError {
    #[error("Failed to type to Url, {0}")]
    UrlConversion(String),

    #[error("Failed to send request, [url: '{url:?}', code: {status:?}, text: '{text}']")]
    Request {
        url: Option<Url>,
        status: Option<StatusCode>,
        text: String,
    },

    #[error("Erroneous response, [url: '{url:?}', code: {status:?}, text: '{text}']")]
    ErroneousRequest {
        url: Option<Url>,
        status: Option<StatusCode>,
        text: String,
    },

    #[error("Failed to deserialize response, [url: '{url:?}', code: {status:?}, text: '{text}']")]
    Deserialize {
        url: Option<Url>,
        status: Option<StatusCode>,
        text: String,
    },

    #[error("Failed to decode hex value to array, {0}")]
    BitcoinHashesHex(#[from] bitcoin_hashes::hex::HexToArrayError),

    #[error("Failed to decode hex value to array, {0}")]
    ElementsHex(simplicityhl::elements::hex::Error),

    #[error("Failed to convert address value to Address, {0}")]
    AddressConversion(String),

    #[error("Failed to decode commitment, type: {commitment_type:?}, error: {error}")]
    CommitmentDecode {
        commitment_type: CommitmentType,
        error: simplicityhl::elements::encode::Error,
    },

    #[error("Failed to decode hex string using hex_simd, error: {0}")]
    HexSimdDecode(
        hex_simd::Error
    ),

    #[error("Failed to deserialize Transaction from hex, error: {0}")]
    TransactionDecode(String),

    #[error(transparent)]
    ElementsRpcError(#[from] corepc_client::client_sync::Error),

    #[error("Elements RPC returned an unexpected value for call {0}")]
    ElementsRpcUnexpectedReturn(String),
}

#[derive(Debug, Clone)]
pub enum CommitmentType {
    Asset,
    Nonce,
    Value,
}

impl ExplorerError {
    #[inline]
    pub(crate) fn response_failed(e: &reqwest::Error) -> Self {
        ExplorerError::Request {
            url: e.url().cloned(),
            status: e.status(),
            text: e.to_string(),
        }
    }

    #[inline]
    pub(crate) fn erroneous_response(e: &reqwest::Response) -> Self {
        ExplorerError::ErroneousRequest {
            url: Some(e.url().clone()),
            status: Some(e.status()),
            text: String::new(),
        }
    }

    #[inline]
    pub(crate) fn deserialize(e: &reqwest::Error) -> Self {
        ExplorerError::Deserialize {
            url: e.url().cloned(),
            status: e.status(),
            text: e.to_string(),
        }
    }
}
