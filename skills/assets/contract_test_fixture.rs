//! Generic offline Simplicity contract execution fixture. Contains no policy expectations.
#![allow(dead_code)]
use simplex::simplicityhl::ast::ElementsJetHinter;
use simplex::simplicityhl::elements::{self, confidential};
use simplex::simplicityhl::simplicity::jet::elements::{ElementsEnv, ElementsUtxo};
use simplex::simplicityhl::simplicity::{BitMachine, Cmr, bitcoin::secp256k1};
use simplex::simplicityhl::{Arguments, TemplateAst, UnstableFeatures, Value, WitnessNameToValueMap, WitnessValues};
use std::{collections::HashMap, sync::Arc};

fn environment(cmr: Cmr, spec: &Spec, mutated: bool) -> ElementsEnv<Arc<elements::Transaction>> {
    let sequence = spec.tx.sequence;
    let mut inputs = vec![elements::TxIn {
        sequence: elements::Sequence(sequence),
        ..Default::default()
    }];
    if let Some(other) = spec.tx.other_sequence {
        inputs.push(elements::TxIn {
            sequence: elements::Sequence(other),
            ..Default::default()
        });
    }
    let utxos = inputs
        .iter()
        .map(|_| ElementsUtxo {
            script_pubkey: elements::Script::new(),
            asset: confidential::Asset::Explicit(elements::AssetId::from_byte_array([1; 32])),
            value: confidential::Value::Explicit(100_000),
        })
        .collect();
    let tx = elements::Transaction {
        version: spec.tx.version,
        lock_time: elements::LockTime::from_consensus(spec.tx.locktime),
        input: inputs,
        output: vec![elements::TxOut {
            asset: confidential::Asset::Explicit(elements::AssetId::from_byte_array([1; 32])),
            value: confidential::Value::Explicit(if mutated { 99_998 } else { 99_999 }),
            ..Default::default()
        }],
    };
    let mut ctrl = vec![0xbe];
    ctrl.extend(hex::decode("50929b74c1a04954b78b4b6035e97a5e078a5a0f28ec96d547bfee9ace803ac0").unwrap());
    ElementsEnv::new(
        Arc::new(tx),
        utxos,
        0,
        cmr,
        elements::taproot::ControlBlock::from_slice(&ctrl).unwrap(),
        None,
        elements::BlockHash::from_byte_array([0; 32]),
    )
}

fn keys() -> Vec<secp256k1::Keypair> {
    let secp = secp256k1::Secp256k1::new();
    (1..=8)
        .map(|n| {
            let mut bytes = [0; 32];
            bytes[31] = n;
            let secret = secp256k1::SecretKey::from_slice(&bytes).unwrap();
            secp256k1::Keypair::from_secret_key(&secp, &secret)
        })
        .collect()
}

/// SHA256 of a deterministic test key's 32-byte x-only encoding, not a compressed key.
pub(crate) fn pubkey_hash(index: usize) -> String {
    assert!((1..=8).contains(&index), "fixture key index must be 1 through 8");
    let key = keys()[index - 1].x_only_public_key().0.serialize();
    format!("0x{}", elements::hashes::sha256::Hash::hash(&key))
}

fn replace(text: &str, keys: &[secp256k1::Keypair], signatures: &[String]) -> String {
    let mut text = text.to_string();
    for (i, key) in keys.iter().enumerate() {
        text = text.replace(&format!("@PK{}", i + 1), &format!("0x{}", key.x_only_public_key().0));
        if let Some(sig) = signatures.get(i) {
            text = text.replace(&format!("@SIG{}", i + 1), sig);
        }
    }
    text.replace("@INVALID", &format!("0x{}", "00".repeat(64)))
}

fn evaluate_inner(template: &TemplateAst, spec: &Spec) -> Result<(), String> {
    let keys = keys();
    let mut arguments = HashMap::new();
    let supplied = &spec.arguments;
    for (name, ty) in template.parameters().iter() {
        let text = supplied
            .get(name.as_ref())
            .map(String::as_str)
            .ok_or_else(|| format!("arguments: missing argument {name}"))?;
        let value = Value::parse_from_str(&replace(text, &keys, &[]), ty).map_err(|e| format!("arguments: {e}"))?;
        arguments.insert(name.clone(), value);
    }
    let program = template
        .instantiate(Arguments::from_map(arguments), false)
        .map_err(|e| format!("arguments: {e}"))?;
    let signing_env = environment(program.commit().cmr(), spec, false);
    let hash = signing_env.c_tx_env().sighash_all().to_byte_array();
    let message = secp256k1::Message::from_digest(hash);
    let secp = secp256k1::Secp256k1::new();
    let signatures: Vec<_> = keys
        .iter()
        .map(|key| format!("0x{}", secp.sign_schnorr_no_aux_rand(&message, key)))
        .collect();
    let supplied = &spec.witnesses;
    let mut witnesses = HashMap::new();
    for (name, ty) in template.witness_types().iter() {
        if let Some(text) = supplied.get(name.as_ref()).map(String::as_str) {
            witnesses.insert(
                name.clone(),
                Value::parse_from_str(&replace(text, &keys, &signatures), ty)
                    .map_err(|e| format!("witness-parsing: {e}"))?,
            );
        }
    }
    let satisfied = program
        .satisfy(WitnessValues::from_map(witnesses))
        .map_err(|e| format!("satisfaction: {e}"))?;
    let env = environment(program.commit().cmr(), spec, spec.tx.mutate_output_after_signing);
    let redeem = satisfied.redeem().prune(&env).map_err(|e| format!("execution: {e}"))?;
    let mut machine = BitMachine::for_program(&redeem).map_err(|e| format!("execution: {e}"))?;
    machine.exec(&redeem, &env).map_err(|e| format!("execution: {e}"))?;
    Ok(())
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Tx {
    pub locktime: u32,
    pub sequence: u32,
    pub version: u32,
    pub other_sequence: Option<u32>,
    pub mutate_output_after_signing: bool,
}
impl Default for Tx {
    fn default() -> Self {
        Self {
            locktime: 0,
            sequence: 0xfffffffe,
            version: 2,
            other_sequence: None,
            mutate_output_after_signing: false,
        }
    }
}
struct Spec {
    tx: Tx,
    arguments: HashMap<String, String>,
    witnesses: HashMap<String, String>,
}
#[derive(Debug)]
pub(crate) struct Failure {
    pub stage: &'static str,
    pub message: String,
}
pub(crate) fn evaluate(
    source: &str,
    arguments: &[(&str, &str)],
    witnesses: &[(&str, &str)],
    tx: Tx,
) -> Result<(), Failure> {
    let template = TemplateAst::new_with_unstable(source, &UnstableFeatures::all(), Box::new(ElementsJetHinter))
        .map_err(|e| Failure {
            stage: "compilation",
            message: e.to_string(),
        })?;
    let pairs = |values: &[(&str, &str)]| values.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    let spec = Spec {
        tx,
        arguments: pairs(arguments),
        witnesses: pairs(witnesses),
    };
    evaluate_inner(&template, &spec).map_err(|message| {
        let stage = if message.starts_with("arguments:") {
            "arguments"
        } else if message.starts_with("witness-parsing:") {
            "witness-parsing"
        } else if message.starts_with("satisfaction:") {
            "satisfaction"
        } else {
            "execution"
        };
        Failure { stage, message }
    })
}
