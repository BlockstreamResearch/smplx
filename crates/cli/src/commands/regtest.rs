use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use simplex_regtest::TestClient;

use crate::commands::error::CommandError;

pub struct Regtest {}

impl Regtest {
    pub fn run() -> Result<(), CommandError> {
        let mut client = TestClient::new();

        let running = Arc::new(AtomicBool::new(true));
        let r = running.clone();

        ctrlc::set_handler(move || {
            r.store(false, Ordering::SeqCst);
        })
        .expect("Error setting Ctrl-C handler");

        println!("======================================");
        println!("Waiting for Ctrl-C...");
        println!("rpc: {}", client.rpc_url());
        println!("esplora: {}", client.esplora_url());
        let auth = client.auth().get_user_pass().unwrap();
        println!("user: {:?}, password: {:?}", auth.0.unwrap(), auth.1.unwrap());
        println!("======================================");

        while running.load(Ordering::SeqCst) {}

        Ok(client.kill()?)
    }
}
