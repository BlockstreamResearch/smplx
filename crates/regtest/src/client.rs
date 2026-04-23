use std::net::TcpListener;
use std::path::{Path, PathBuf};

use electrsd::ElectrsD;
use electrsd::bitcoind;
use electrsd::bitcoind::bitcoincore_rpc::Auth;
use electrsd::bitcoind::{BitcoinD, Conf};

use super::RegtestConfig;
use super::error::RegtestError;
use crate::args::{get_electrs_bin_args, get_elementsd_bin_args};

pub struct RegtestClient {
    pub electrs: ElectrsD,
    pub elements: BitcoinD,
    config: RegtestConfig,
}

impl RegtestClient {
    pub fn new(config: &RegtestConfig) -> Self {
        let (electrs_path, elementsd_path) = Self::default_bin_paths();
        let zmq_addr = Self::get_zmq_addr();
        let elements = Self::create_bitcoind_node(elementsd_path, &zmq_addr, config);
        let electrs = Self::create_electrs_node(electrs_path, &elements, &zmq_addr, config);

        Self {
            electrs,
            elements,
            config: config.clone(),
        }
    }

    pub fn default_bin_paths() -> (PathBuf, PathBuf) {
        const ELECTRS_BIN_PATH: &str = "electrs";
        const ELEMENTSD_BIN_PATH: &str = "elementsd";

        (
            Path::new(ELECTRS_BIN_PATH).to_path_buf(),
            Path::new(ELEMENTSD_BIN_PATH).to_path_buf(),
        )
    }

    pub fn rpc_url(&self) -> String {
        if let Some(port) = self.config.rpc_port {
            return format!("http://127.0.0.1:{port}");
        }

        self.elements.rpc_url()
    }

    pub fn esplora_url(&self) -> String {
        if let Some(port) = self.config.esplora_port {
            return format!("http://127.0.0.1:{port}");
        }

        let url = self.electrs.esplora_url.clone().unwrap();
        let port = url.split_once(":").unwrap().1;

        format!("http://127.0.0.1:{}", port)
    }

    pub fn auth(&self) -> Auth {
        if let (Some(user), Some(password)) = (&self.config.rpc_user, &self.config.rpc_password) {
            return Auth::UserPass(user.clone(), password.clone());
        }

        let cookie = self.elements.params.get_cookie_values().unwrap().unwrap();

        Auth::UserPass(cookie.user, cookie.password)
    }

    pub fn kill(&mut self) -> Result<(), RegtestError> {
        // electrs stops elements automatically
        self.electrs.kill().map_err(|_| RegtestError::ElectrsTermination())?;

        Ok(())
    }

    fn get_zmq_addr() -> String {
        TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .to_string()
    }

    fn create_bitcoind_node(bin_path: impl AsRef<Path>, zmq_addr: &String, config: &RegtestConfig) -> BitcoinD {
        let mut conf = Conf::default();
        let mut bin_args = get_elementsd_bin_args(config);

        bin_args.push(format!("-zmqpubrawtx=tcp://{zmq_addr}"));
        bin_args.push(format!("-zmqpubrawblock=tcp://{zmq_addr}"));
        bin_args.push(format!("-zmqpubhashtx=tcp://{zmq_addr}"));
        bin_args.push(format!("-zmqpubhashblock=tcp://{zmq_addr}"));
        bin_args.push(format!("-zmqpubsequence=tcp://{zmq_addr}"));

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.network = "liquidregtest";
        conf.p2p = bitcoind::P2P::Yes;

        BitcoinD::with_conf(bin_path.as_ref(), &conf).unwrap()
    }

    fn create_electrs_node(
        bin_path: impl AsRef<Path>,
        elementsd: &BitcoinD,
        zmq_addr: &String,
        config: &RegtestConfig,
    ) -> ElectrsD {
        let mut conf = electrsd::Conf::default();
        let mut bin_args = get_electrs_bin_args(config);

        bin_args.push(format!("--zmq-addr={zmq_addr}"));

        conf.args = bin_args.iter().map(|x| x.as_ref()).collect::<Vec<&str>>();
        conf.http_enabled = config.esplora_port.is_none();
        conf.network = "liquidregtest";

        ElectrsD::with_conf(bin_path.as_ref(), elementsd, &conf).unwrap()
    }
}
