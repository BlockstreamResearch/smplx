pub trait ElementsdParams {
    fn get_bin_args(&self) -> Vec<String>;
}

pub struct DefaultElementsdParams;

impl ElementsdParams for DefaultElementsdParams {
    fn get_bin_args(&self) -> Vec<String> {
        vec![
            "-fallbackfee=0.0001".to_string(),
            "-dustrelayfee=0.00000001".to_string(),
            "-initialfreecoins=2100000000".to_string(),
            "-acceptdiscountct=1".to_string(),
            "-rest".to_string(),
            "-evbparams=simplicity:-1:::".to_string(), // Enable Simplicity from block 0
            "-minrelaytxfee=0".to_string(),            // test tx with no fees/asset fees
            "-blockmintxfee=0".to_string(),            // test tx with no fees/asset fees
            "-chain=liquidregtest".to_string(),
            "-txindex=1".to_string(),
            "-validatepegin=0".to_string(),
            "-initialfreecoins=2100000000000000".to_string(),
            "-listen=1".to_string(),
            "-txindex=1".to_string(),
        ]
    }
}
