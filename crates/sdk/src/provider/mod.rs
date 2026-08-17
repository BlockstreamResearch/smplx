/// Core provider traits and information structs used to define general blockchain interaction interfaces.
pub mod core;
/// Provider-specific error enumerations for handling transmission, retrieval, or interpretation issues.
pub mod error;
/// Types and definitions for interacting specifically with an Esplora REST API provider backend.
#[cfg(feature = "provider")]
pub mod esplora;
/// Definitions distinguishing blockchain network states (e.g. mainnet, testnet, regtest) and related configurations.
pub mod network;
/// Submodules and definitions handling direct JSON-RPC interfacing with backing Bitcoin/Elements core nodes.
#[cfg(feature = "provider")]
pub mod rpc;
/// Abstractions and composite providers intended for general usage in the Simplex SDK.
#[cfg(feature = "provider")]
pub mod simplex;

#[cfg(feature = "provider")]
pub use core::ProviderInfo;
pub use core::ProviderTrait;
#[cfg(feature = "provider")]
pub use esplora::EsploraProvider;
#[cfg(feature = "provider")]
pub use rpc::elements::ElementsRpc;
#[cfg(feature = "provider")]
pub use simplex::SimplexProvider;

pub use network::*;

pub use error::ProviderError;
#[cfg(feature = "provider")]
pub use rpc::error::RpcError;
