use lwk_wollet::ElementsNetwork;
use smplx_sdk::provider::SimplicityNetwork;

use crate::error::TestError;

pub(crate) fn to_elements_network(network: &SimplicityNetwork) -> Result<ElementsNetwork, TestError> {
    match network {
        SimplicityNetwork::Liquid => Ok(ElementsNetwork::Liquid),
        SimplicityNetwork::LiquidTestnet => Ok(ElementsNetwork::LiquidTestnet),
        SimplicityNetwork::ElementsRegtest { policy_asset } => Ok(ElementsNetwork::ElementsRegtest {
            policy_asset: policy_asset
                .to_string()
                .parse::<lwk_wollet::elements::AssetId>()
                .map_err(|error| TestError::WalletAbiInvariant(error.to_string()))?,
        }),
    }
}
