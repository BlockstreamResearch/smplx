use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use simplex_regtest::Regtest as RegtestRunner;
use simplex_regtest::RegtestConfig;

use crate::commands::error::CommandError;

pub struct Regtest {}

impl Regtest {
    pub fn run(config: RegtestConfig) -> Result<(), CommandError> {
        let (mut client, signer) = RegtestRunner::new(config)?;

        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");

        let auth = client.auth().get_user_pass().unwrap();

        println!("======================================");
        println!("Waiting for Ctrl-C...");
        println!();
        println!("RPC: {}", client.rpc_url());
        println!("Esplora: {}", client.esplora_url());
        println!("User: {:?}, Password: {:?}", auth.0.unwrap(), auth.1.unwrap());
        println!();
        println!("Signer: {:?}", signer.get_wpkh_address()?);
        println!("======================================");

        while running.load(Ordering::SeqCst) {}

        Ok(client.kill()?)
    }
}
