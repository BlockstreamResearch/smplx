pub const DEFAULT_SAT_AMOUNT_FAUCET: u64 = 100000;

pub fn get_elementsd_bin_args() -> Vec<String> {
    vec![
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
    ]
}

pub fn get_electrs_bin_args() -> Vec<String> {
    vec!["-v".to_string()]
}
