//! Run with `cargo test -p smplx-sdk --test automatic_padding_regtest -- --ignored`.
//! Requires `elementsd` and `elements-cli` on PATH. All traffic stays on loopback.

use std::collections::HashMap;
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value as Json, json};
use simplicityhl::elements::{OutPoint, Transaction, confidential, encode};
use simplicityhl::str::WitnessName;
use simplicityhl::value::ValueConstructible;
use simplicityhl::{Arguments, Value, WitnessValues};
use smplx_sdk::program::{ArgumentsTrait, Program, WitnessTrait};
use smplx_sdk::provider::SimplicityNetwork;
use smplx_sdk::signer::Signer;
use smplx_sdk::transaction::{ChangeOutput, FinalTransaction, PartialInput, ProgramInput, RequiredSignature, UTXO};

struct Node {
    child: Child,
    dir: PathBuf,
    port: u16,
}

impl Node {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let dir = std::env::temp_dir().join(format!("smplx-padding-{}-{nonce}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let child = Command::new("elementsd")
            .arg(format!("-datadir={}", dir.display()))
            .arg(format!("-rpcport={port}"))
            .args([
                "-chain=liquidregtest",
                "-server=1",
                "-listen=0",
                "-connect=0",
                "-discover=0",
                "-dnsseed=0",
                "-validatepegin=0",
                "-initialfreecoins=2100000000000000",
                "-evbparams=simplicity:-1:::",
                "-acceptnonstdtxn=0",
                "-acceptdiscountct=1",
                "-fallbackfee=0.00001",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("elementsd must be installed on PATH");
        let mut node = Self { child, dir, port };
        for _ in 0..100 {
            if node.try_rpc(&["getblockchaininfo"]).is_ok() {
                return node;
            }
            assert!(
                node.child.try_wait().unwrap().is_none(),
                "elementsd exited during startup; inspect {}",
                node.dir.display()
            );
            sleep(Duration::from_millis(100));
        }
        panic!("elementsd did not become ready; inspect {}", node.dir.display());
    }

    fn try_rpc(&self, args: &[&str]) -> Result<Json, String> {
        let output = Command::new("elements-cli")
            .arg(format!("-datadir={}", self.dir.display()))
            .arg(format!("-rpcport={}", self.port))
            .arg("-chain=liquidregtest")
            .args(args)
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        let text = String::from_utf8(output.stdout).unwrap();
        Ok(serde_json::from_str(&text).unwrap_or_else(|_| Json::String(text.trim().to_owned())))
    }

    fn rpc(&self, args: &[&str]) -> Json {
        self.try_rpc(args)
            .unwrap_or_else(|e| panic!("RPC {} failed: {e}", args[0]))
    }

    fn accepts(&self, tx: &Transaction) -> Json {
        let raw = json!([hex::encode(encode::serialize(tx))]).to_string();
        self.rpc(&["testmempoolaccept", &raw])[0].clone()
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = self.try_rpc(&["stop"]);
        for _ in 0..50 {
            if self.child.try_wait().ok().flatten().is_some() {
                let _ = std::fs::remove_dir_all(&self.dir);
                return;
            }
            sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[derive(Clone)]
struct EmptyArguments;
impl ArgumentsTrait for EmptyArguments {
    fn build_arguments(&self) -> Arguments {
        Arguments::default()
    }
}

#[derive(Clone)]
struct SignatureWitness;
impl WitnessTrait for SignatureWitness {
    fn build_witness(&self) -> WitnessValues {
        WitnessValues::from(HashMap::from([(
            WitnessName::from_str_unchecked("SIGNATURE"),
            Value::byte_array([0u8; 64]),
        )]))
    }
}

#[test]
#[ignore = "requires local Elements binaries; starts an isolated node with standardness enabled"]
fn padded_signature_spend_passes_standard_policy() {
    let node = Node::start();
    let network = SimplicityNetwork::default_regtest();
    assert_eq!(
        node.rpc(&["getblockhash", "0"]).as_str().unwrap(),
        network.genesis_block_hash().to_string()
    );
    let signer = Signer::from_mnemonic(
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about",
        network,
    );
    let public_key = hex::encode(signer.get_schnorr_public_key().serialize());
    let mut source = format!(
        "fn main() {{ let pk: Pubkey = 0x{public_key}; let msg: u256 = jet::sig_all_hash(); let sig: Signature = witness::SIGNATURE;"
    );
    for _ in 0..64 {
        source.push_str("jet::bip_0340_verify((pk, msg), sig);\n");
    }
    source.push('}');
    let program = signer.prepare_program(Program::new(source, &EmptyArguments)).unwrap();
    let address = program.get_tr_address(&network).to_string();

    // Regtest's initial policy-asset output is anyone-can-spend. Funding only touches this isolated node.
    let genesis = node.rpc(&["getblock", &network.genesis_block_hash().to_string(), "2"]);
    let input = json!([{"txid": genesis["tx"][1]["txid"], "vout": 0}]).to_string();
    let outputs = json!([{address: 20999999.99999}, {"fee": 0.00001}]).to_string();
    let funding_hex = node
        .rpc(&["createrawtransaction", &input, &outputs])
        .as_str()
        .unwrap()
        .to_owned();
    let funding: Transaction = encode::deserialize(&hex::decode(&funding_hex).unwrap()).unwrap();
    node.rpc(&["sendrawtransaction", &funding_hex]);
    node.rpc(&["generatetoaddress", "1", &signer.get_address().to_string()]);
    let (vout, output) = funding
        .output
        .iter()
        .enumerate()
        .find(|(_, o)| o.script_pubkey == program.get_script_pubkey(&network))
        .unwrap();
    let mut tx = FinalTransaction::new();
    tx.add_program_input(
        PartialInput::new(UTXO {
            outpoint: OutPoint::new(funding.txid(), vout as u32),
            txout: output.clone(),
            secrets: None,
        }),
        ProgramInput::new(Box::new(program), Box::new(SignatureWitness)),
        RequiredSignature::Witness("SIGNATURE".to_owned()),
    );
    tx.add_change(ChangeOutput::new(signer.get_address().script_pubkey()));
    let (signed, fee) = signer.finalize_strict(&tx, 1000.0).unwrap();
    assert_eq!(
        signed.input[0].witness.script_witness.len(),
        4,
        "padding must not use an annex"
    );
    assert!(
        signed.input[0].witness.script_witness[0].len() > 64,
        "fixture must need padding"
    );
    let result = node.accepts(&signed);
    assert_eq!(result["allowed"], true, "{result}");
    eprintln!(
        "standard-policy accepted: witness={} bytes, tx={} bytes, weight={}, discount_weight={}, fee={fee}",
        signed.input[0].witness.script_witness[0].len(),
        encode::serialize(&signed).len(),
        signed.weight(),
        signed.discount_weight()
    );

    let mut bad_witness = signed.clone();
    bad_witness.input[0].witness.script_witness[0][0] ^= 1;
    let result = node.accepts(&bad_witness);
    assert_eq!(result["allowed"], false, "altered witness accepted: {result}");

    let mut bad_signature = signed;
    let change = bad_signature
        .output
        .iter()
        .position(|o| !o.script_pubkey.is_empty())
        .unwrap();
    let fee_index = bad_signature
        .output
        .iter()
        .position(|o| o.script_pubkey.is_empty())
        .unwrap();
    bad_signature.output[change].value =
        confidential::Value::Explicit(bad_signature.output[change].value.explicit().unwrap() - 1);
    bad_signature.output[fee_index].value =
        confidential::Value::Explicit(bad_signature.output[fee_index].value.explicit().unwrap() + 1);
    let result = node.accepts(&bad_signature);
    assert_eq!(result["allowed"], false, "changed signed outputs accepted: {result}");
}
