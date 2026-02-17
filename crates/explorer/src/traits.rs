use serde::{Deserialize, Serialize};

pub trait RpcBackend {

}


#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Liquid,
    LiquidTestnet,
    ElementsRegtest,
}
