use std::collections::BTreeMap;
use std::sync::Arc;

use simplicityhl::ast::ElementsJetHinter;
use simplicityhl::{Arguments, TemplateAst, UnstableFeatures, WitnessValues};

use smplx_sdk::program::Program as SdkProgram;

use wasm_bindgen::prelude::*;

use crate::network::network_from_str;

/// Compile-time parameters for a program, resolved before construction.
#[derive(Clone)]
struct FixedArguments(Arguments);

impl From<FixedArguments> for Arguments {
    fn from(val: FixedArguments) -> Self {
        val.0
    }
}

impl From<&FixedArguments> for Arguments {
    fn from(val: &FixedArguments) -> Self {
        val.0.clone()
    }
}

/// Witness values for a program input, resolved before the transaction is assembled.
#[derive(Clone)]
pub(crate) struct FixedWitness(pub(crate) WitnessValues);

impl From<FixedWitness> for WitnessValues {
    fn from(val: FixedWitness) -> Self {
        val.0
    }
}

impl From<&FixedWitness> for WitnessValues {
    fn from(val: &FixedWitness) -> Self {
        val.0.clone()
    }
}

/// A compiled `SimplicityHL` program.
#[wasm_bindgen]
pub struct Program {
    pub(crate) program: SdkProgram,
}

#[wasm_bindgen]
impl Program {
    /// Creates a program from `SimplicityHL` source text delivered at runtime.
    ///
    /// `argumentsJson` carries the program's compile-time parameters.
    ///
    /// Shape: `{"NAME": {"value": "0x…", "type": "Pubkey"}}`.
    /// Pass `None` for a program that declares no parameters.
    ///
    /// `extraLeavesJson` is a JSON array of hex strings, each an encoded taproot
    /// leaf payload appended to the tree in declaration order.
    ///
    /// # Errors
    /// Returns an error if the arguments are not valid `SimplicityHL` argument JSON, or if the
    /// extra leaves are not a JSON array of 32-byte hex strings.
    #[wasm_bindgen(constructor)]
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        source: &str,
        arguments_json: Option<String>,
        extra_leaves_json: Option<String>,
        include_debug_symbols: Option<bool>,
    ) -> Result<Program, JsError> {
        Ok(Self {
            program: Self::from_source(
                source,
                arguments_json.as_deref(),
                extra_leaves_json.as_deref(),
                include_debug_symbols,
            )?,
        })
    }

    /// Compiles the program and returns its Commitment Merkle Root as lowercase hex.
    ///
    /// # Errors
    /// Returns an error if the source fails to compile.
    #[wasm_bindgen(js_name = cmr)]
    pub fn cmr(&self) -> Result<String, JsError> {
        self.validate()?;
        let cmr = self.program.get_cmr();

        Ok(hex::encode(cmr))
    }

    /// Compiles the program and returns the tapleaf hash of its Simplicity script, as hex.
    ///
    /// # Errors
    /// Returns an error if the source fails to compile.
    #[wasm_bindgen(js_name = tapleafHash)]
    pub fn tapleaf_hash(&self) -> Result<String, JsError> {
        self.validate()?;

        Ok(hex::encode(self.program.get_tapleaf_hash()))
    }

    /// Compiles the program and returns the `scriptPubKey` its funds are locked with, as hex.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptPubkey)]
    pub fn script_pubkey(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;
        self.validate()?;

        Ok(hex::encode(self.program.get_script_pubkey(&network).as_bytes()))
    }

    /// Compiles the program and returns its `scriptPubKey` hash.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = scriptHash)]
    pub fn script_hash(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;
        self.validate()?;

        Ok(hex::encode(self.program.get_script_hash(&network)))
    }

    /// Compiles the program and returns the taproot address its funds would sit at.
    ///
    /// # Errors
    /// Returns an error if the network name is unknown or the source fails to compile.
    #[wasm_bindgen(js_name = trAddress)]
    pub fn tr_address(&self, network: &str) -> Result<String, JsError> {
        let network = network_from_str(network)?;
        self.validate()?;

        Ok(self.program.get_tr_address(&network).to_string())
    }

    fn from_source(
        source: &str,
        arguments_json: Option<&str>,
        extra_leaves_json: Option<&str>,
        include_debug_symbols: Option<bool>,
    ) -> Result<SdkProgram, JsError> {
        let arguments = match arguments_json {
            Some(json) if !json.trim().is_empty() => serde_json::from_str::<Arguments>(json)
                .map_err(|e| JsError::new(&format!("Invalid program arguments: {e}")))?,
            _ => Arguments::default(),
        };

        let mut program = SdkProgram::new(Arc::<str>::from(source), FixedArguments(arguments));

        if let Some(include) = include_debug_symbols {
            program = program.with_debug_symbols(include);
        }

        if let Some(json) = extra_leaves_json.filter(|json| !json.trim().is_empty()) {
            let leaves = Self::state_leaves(json).map_err(|e| JsError::new(&e))?;

            program = program.with_storage_capacity(leaves.len());

            for (index, leaf) in leaves.into_iter().enumerate() {
                program.set_storage_at(index, leaf);
            }
        }

        Ok(program)
    }

    fn state_leaves(json: &str) -> Result<Vec<Vec<u8>>, String> {
        let declared: Vec<String> = serde_json::from_str(json).map_err(|e| format!("Invalid extra leaves: {e}"))?;

        if declared.len() > SdkProgram::MAX_STORAGE_SLOTS {
            return Err(format!(
                "A program holds at most {} extra leaves, and this one has {}",
                SdkProgram::MAX_STORAGE_SLOTS,
                declared.len()
            ));
        }
        let mut leaves = Vec::with_capacity(declared.len());

        for (index, leaf) in declared.iter().enumerate() {
            let bytes = hex::decode(leaf.strip_prefix("0x").unwrap_or(leaf))
                .map_err(|e| format!("Extra leaf {index} is not hex: {e}"))?;

            if bytes.len() != SdkProgram::STORAGE_SLOT_BYTES {
                return Err(format!(
                    "Extra leaf {index} is {} bytes, and a leaf is {}. A program reads its state in \
                     {}-byte steps, so a leaf of any other width commits to something the program \
                     cannot read back.",
                    bytes.len(),
                    SdkProgram::STORAGE_SLOT_BYTES,
                    SdkProgram::STORAGE_SLOT_BYTES,
                ));
            }

            leaves.push(bytes);
        }

        Ok(leaves)
    }

    fn validate(&self) -> Result<(), JsError> {
        self.program
            .get_argument_types()
            .map(|_| ())
            .map_err(|e| JsError::new(&format!("Program does not compile: {e}")))
    }
}

/// The compile-time parameters a program source declares, as JSON of name to type.
///
/// # Errors
/// Returns an error if the source does not parse or does not type-check.
#[wasm_bindgen(js_name = programArgumentTypes)]
pub fn program_argument_types(source: &str) -> Result<String, JsError> {
    let template = TemplateAst::new_with_unstable(
        Arc::<str>::from(source),
        &UnstableFeatures::all(),
        Box::new(ElementsJetHinter),
    )
    .map_err(|e| JsError::new(&format!("Program does not compile: {e}")))?;

    let declared: BTreeMap<String, String> = template
        .parameters()
        .iter()
        .map(|(name, ty)| (name.as_ref().to_string(), ty.to_string()))
        .collect();

    serde_json::to_string(&declared).map_err(|e| JsError::new(&format!("Cannot report parameter types: {e}")))
}

#[cfg(test)]
mod tests {
    use super::Program;

    const TRIVIAL: &str = "fn main() { }";

    #[test]
    fn short_state_leaf_is_rejected() {
        let refused = Program::state_leaves("[\"00\"]").expect_err("a one byte leaf");

        assert!(refused.contains("is 1 bytes, and a leaf is 32"), "{refused}");
    }

    #[test]
    fn non_hex_state_leaf_is_rejected() {
        assert!(Program::state_leaves("[\"nonsense\"]").is_err());
    }

    #[test]
    fn state_leaves_keep_their_order() {
        let read = Program::state_leaves(&format!("[\"{}\", \"0x{}\"]", "00".repeat(32), "11".repeat(32)))
            .expect("two full leaves");

        assert_eq!(read, vec![vec![0x00; 32], vec![0x11; 32]]);
    }

    #[test]
    fn state_leaf_changes_the_address_but_not_the_cmr() {
        let leaf = "00".repeat(32);
        let other = format!("{}01", "00".repeat(31));

        let plain = Program::new(TRIVIAL, None, None, None).expect("a program that compiles");
        let stateful = Program::new(TRIVIAL, None, Some(format!("[\"{leaf}\"]")), None).expect("a program with state");
        let moved = Program::new(TRIVIAL, None, Some(format!("[\"{other}\"]")), None).expect("a program with state");

        let network = "liquidtestnet";

        assert_ne!(
            plain.tr_address(network).unwrap(),
            stateful.tr_address(network).unwrap()
        );
        assert_ne!(
            stateful.tr_address(network).unwrap(),
            moved.tr_address(network).unwrap()
        );
        // The program is the same, only the address changes.
        assert_eq!(stateful.cmr().unwrap(), moved.cmr().unwrap());
        assert_eq!(stateful.tapleaf_hash().unwrap(), moved.tapleaf_hash().unwrap());
    }
}
