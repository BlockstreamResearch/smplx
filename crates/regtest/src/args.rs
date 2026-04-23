use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::RegtestConfig;

type HmacSha256 = Hmac<Sha256>;

pub fn get_elementsd_bin_args(config: &RegtestConfig) -> Vec<String> {
    let mut args = vec![
        "-fallbackfee=0.0001".to_string(),
        "-dustrelayfee=0.00000001".to_string(),
        "-acceptdiscountct=1".to_string(),
        "-rest".to_string(),
        "-evbparams=simplicity:-1:::".to_string(),
        "-minrelaytxfee=0".to_string(),
        "-blockmintxfee=0".to_string(),
        "-chain=liquidregtest".to_string(),
        "-txindex=1".to_string(),
        "-validatepegin=0".to_string(),
        "-initialfreecoins=2100000000000000".to_string(),
        "-listen=1".to_string(),
        "-txindex=1".to_string(),
        "-multi_data_permitted".to_string(),
    ];

    if let Some(port) = config.rpc_port {
        // A lib works with autoassigned rpcport, so we are required to do this woodoo
        args.push("-rpcbind=127.0.0.1".to_string());
        args.push(format!("-rpcbind=127.0.0.1:{port}"));
        args.push("-rpcallowip=127.0.0.1/8".to_string());
    }

    if let (Some(user), Some(password)) = (&config.rpc_user, &config.rpc_password) {
        let rpcauth = generate_rpcauth(user, password);
        args.push(format!("-rpcauth={rpcauth}"));
    }

    args
}

pub fn get_electrs_bin_args(config: &RegtestConfig) -> Vec<String> {
    let mut args = vec![];

    if let Some(port) = config.esplora_port {
        args.push(format!("--http-addr=0.0.0.0:{port}"));
    }

    args
}

/// Generates an rpcauth string in the format `user:salt$hash`
fn generate_rpcauth(user: &str, password: &str) -> String {
    let salt = hex::encode(user.as_bytes());
    let mut mac = HmacSha256::new_from_slice(salt.as_bytes()).expect("HMAC accepts key of any size");

    mac.update(password.as_bytes());

    let hash = hex::encode(mac.finalize().into_bytes());

    format!("{user}:{salt}${hash}")
}
