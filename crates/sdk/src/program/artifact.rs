//! Source-free program artifact.
//!
//! A [`Program`](super::core::Program) derives its address and satisfies a spend from
//! an [`Artifact`] rather than the linked SimplicityHL frontend. The artifact holds
//! the CMR (the address), the consensus-serialized commit DAG, and the witness
//! layout, produced by the pinned out-of-process `simc`. [`Artifact::from_json`]
//! parses `simc --json`; [`Artifact::from_compiled`] freezes an in-process
//! `CompiledProgram` for the differential tests.

use std::sync::Arc;

use simplicityhl::debug::DebugSymbols;
use simplicityhl::simplicity::dag::{InternalSharing, PostOrderIterItem};
use simplicityhl::simplicity::jet::Elements;
use simplicityhl::simplicity::node::{self, Converter, Inner, NoWitness};
use simplicityhl::simplicity::{BitIter, Cmr, CommitNode, RedeemNode, Value};
use simplicityhl::str::WitnessName;
use simplicityhl::value::StructuralValue;
use simplicityhl::{CompiledProgram, WitnessValues};

/// A frozen, source-free view of a compiled Simplicity program: enough to derive
/// the address and satisfy a spend without re-running the SimplicityHL frontend.
#[derive(Clone, Debug)]
pub struct Artifact {
    /// Commitment Merkle Root: the program's identity and the basis of its address.
    cmr: Cmr,
    /// The commit DAG, decoded once and shared by every satisfaction.
    commit: Arc<CommitNode>,
    /// Witness node names in DAG post order, the order [`Self::satisfy`] assigns
    /// values in. Frozen from `CompiledProgram::witness_layout`.
    witness_names: Vec<WitnessName>,
    /// CMR-keyed debug symbols, present for a `--debug` compile. Instrumentation is
    /// part of the program, so the symbols describe exactly this artifact.
    debug_symbols: Option<DebugSymbols>,
}

impl Artifact {
    /// Freeze a compiled program into a source-free artifact.
    #[must_use]
    pub fn from_compiled(compiled: &CompiledProgram) -> Self {
        let commit = compiled.commit();
        Self {
            cmr: commit.cmr(),
            commit,
            witness_names: compiled.witness_layout().into_iter().map(|(name, _ty)| name).collect(),
            debug_symbols: Some(compiled.debug_symbols().clone()),
        }
    }

    /// Build an artifact from the JSON output of `simc --json`.
    ///
    /// The CMR is recomputed from the decoded program bytes and cross-checked against
    /// the `cmr` field, so a corrupt output is rejected here rather than surfacing
    /// later as a wrong address.
    ///
    /// # Errors
    /// Returns an error if the JSON is malformed, the program bytes fail to decode,
    /// or the recomputed CMR disagrees with the declared one.
    pub fn from_json(json: &str) -> Result<Self, String> {
        use base64::Engine as _;
        use serde::Deserialize;

        #[derive(Deserialize)]
        struct Output {
            program: String,
            cmr: String,
            witness_layout: Vec<Entry>,
            /// Present only for `--debug` compiles; older compilers never emit it.
            #[serde(default)]
            debug_symbols: Option<DebugSymbols>,
        }
        #[derive(Deserialize)]
        struct Entry {
            name: String,
        }

        let output: Output = serde_json::from_str(json).map_err(|e| e.to_string())?;
        let commit_bytes = base64::engine::general_purpose::STANDARD
            .decode(&output.program)
            .map_err(|e| format!("program is not valid base64: {e}"))?;

        let commit = CommitNode::decode::<_, Elements>(BitIter::from(&commit_bytes[..])).map_err(|e| e.to_string())?;
        let cmr = commit.cmr();
        if hex::encode(cmr.as_ref()) != output.cmr {
            return Err(format!(
                "declared CMR {} does not match the program bytes (CMR {})",
                output.cmr,
                hex::encode(cmr.as_ref())
            ));
        }

        let witness_names = output
            .witness_layout
            .into_iter()
            .map(|entry| WitnessName::from_str_unchecked(&entry.name))
            .collect();

        Ok(Self {
            cmr,
            commit,
            witness_names,
            debug_symbols: output.debug_symbols,
        })
    }

    /// The program's CMR. The Taproot leaf script is `Script::from(cmr)`, so the
    /// address and control block derive from this alone.
    #[must_use]
    pub fn cmr(&self) -> Cmr {
        self.cmr
    }

    /// The program's debug symbols, when it was compiled with debug instrumentation.
    #[must_use]
    pub fn debug_symbols(&self) -> Option<&DebugSymbols> {
        self.debug_symbols.as_ref()
    }

    /// Reconstruct the redeem program from the frozen commit DAG and the witness
    /// values, without the SimplicityHL frontend.
    ///
    /// Mirrors `CompiledProgram::satisfy`, but assigns witness values by post-order
    /// index in the decoded commit DAG instead of by name. Each value is type-checked
    /// against its witness node, so a wrong-typed value fails here with the witness
    /// named rather than misbehaving in the bit machine. The result is not pruned.
    ///
    /// # Errors
    /// Returns an error if a witness value is missing or has the wrong type, or the
    /// number of witness nodes disagrees with the frozen layout.
    pub fn satisfy(&self, witness: &WitnessValues) -> Result<Arc<RedeemNode>, String> {
        // Structural values in layout order, converted exactly as `satisfy` does.
        let values = self
            .witness_names
            .iter()
            .map(|name| {
                witness
                    .get(name)
                    .map(|value| Value::from(StructuralValue::from(value)))
                    .ok_or_else(|| format!("missing witness for {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut populator = IndexPopulator {
            names: &self.witness_names,
            values,
            next: 0,
        };
        let redeem = self.commit.convert::<InternalSharing, _, _>(&mut populator)?;

        if populator.next != populator.values.len() {
            return Err(format!(
                "witness layout has {} entries but the program has {} witness nodes",
                populator.values.len(),
                populator.next
            ));
        }
        Ok(redeem)
    }
}

/// Assigns the `k`-th witness node encountered in post order the `k`-th value.
/// The name-free counterpart of SimplicityHL's `named::populate_witnesses`.
struct IndexPopulator<'a> {
    names: &'a [WitnessName],
    values: Vec<Value>,
    next: usize,
}

impl Converter<node::Commit, node::Redeem> for IndexPopulator<'_> {
    type Error = String;

    fn convert_witness(&mut self, item: &PostOrderIterItem<&CommitNode>, _: &NoWitness) -> Result<Value, Self::Error> {
        let index = self.next;
        let value = self
            .values
            .get(index)
            .cloned()
            .ok_or("more witness nodes than layout entries")?;
        self.next += 1;

        let target = &item.node.cached_data().arrow().target;
        if !value.is_of_type(target) {
            let name = self
                .names
                .get(index)
                .map_or_else(|| index.to_string(), ToString::to_string);
            return Err(format!(
                "witness {name} has type {} but the program expects {target}",
                value.ty()
            ));
        }
        Ok(value)
    }

    fn convert_disconnect(
        &mut self,
        _: &PostOrderIterItem<&CommitNode>,
        _: Option<&Arc<RedeemNode>>,
        _: &node::NoDisconnect,
    ) -> Result<Arc<RedeemNode>, Self::Error> {
        unreachable!("SimplicityHL does not use disconnect right now")
    }

    fn convert_data(
        &mut self,
        data: &PostOrderIterItem<&CommitNode>,
        inner: Inner<&Arc<RedeemNode>, &Arc<RedeemNode>, &Value>,
    ) -> Result<Arc<node::RedeemData>, Self::Error> {
        let inner = inner
            .map(|node| node.cached_data())
            .map_disconnect(|node| node.cached_data())
            .map_witness(Value::shallow_clone);
        Ok(Arc::new(node::RedeemData::new(
            data.node.cached_data().arrow().shallow_clone(),
            inner,
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use simplicityhl::ast::ElementsJetHinter;
    use simplicityhl::value::UIntValue;
    use simplicityhl::{Arguments, UnstableFeatures, Value};

    use super::*;

    fn compile(src: &str) -> CompiledProgram {
        CompiledProgram::new_with_unstable(
            src,
            &UnstableFeatures::all(),
            Arguments::default(),
            false,
            Box::new(ElementsJetHinter),
        )
        .expect("program compiles")
    }

    fn witness(pairs: &[(&str, Value)]) -> WitnessValues {
        let map: HashMap<WitnessName, Value> = pairs
            .iter()
            .map(|(name, value)| (WitnessName::from_str_unchecked(name), value.clone()))
            .collect();
        WitnessValues::from(map)
    }

    // The artifact path (freeze, decode, satisfy-by-index) reconstructs a redeem
    // program byte-for-byte identical to `CompiledProgram::satisfy`, and freezes the
    // same CMR.
    #[test]
    fn artifact_matches_frontend_bit_for_bit() {
        let src = "fn main() {
            let a: u16 = witness::A;
            let b: u32 = witness::B;
            assert!(jet::eq_16(a, 7));
            assert!(jet::eq_32(b, 9));
        }";
        let compiled = compile(src);
        let witness = witness(&[
            ("A", Value::from(UIntValue::U16(7))),
            ("B", Value::from(UIntValue::U32(9))),
        ]);

        // Frontend path (unpruned redeem), the ground truth.
        let expected = compiled
            .satisfy(witness.clone())
            .expect("frontend satisfies")
            .redeem()
            .to_vec_with_witness();

        // Artifact path: no source, no frontend.
        let artifact = Artifact::from_compiled(&compiled);
        assert_eq!(artifact.cmr(), compiled.commit().cmr(), "address (CMR) must match");

        let redeem = artifact.satisfy(&witness).expect("artifact satisfies");
        assert_eq!(
            redeem.to_vec_with_witness(),
            expected,
            "artifact-reconstructed spend must be byte-identical to the frontend spend"
        );
    }

    // A missing witness value is reported, not silently mis-satisfied.
    #[test]
    fn artifact_reports_missing_witness() {
        let compiled = compile("fn main() { let a: u16 = witness::A; assert!(jet::eq_16(a, 7)); }");
        let artifact = Artifact::from_compiled(&compiled);
        let err = artifact.satisfy(&witness(&[])).unwrap_err();
        assert!(err.contains("missing witness"), "got: {err}");
    }

    // A wrong-typed witness value is rejected with the witness named, not fed into
    // the bit machine.
    #[test]
    fn artifact_rejects_wrong_typed_witness() {
        let compiled = compile("fn main() { let a: u16 = witness::A; assert!(jet::eq_16(a, 7)); }");
        let artifact = Artifact::from_compiled(&compiled);
        let err = artifact
            .satisfy(&witness(&[("A", Value::from(UIntValue::U32(7)))]))
            .unwrap_err();
        assert!(err.contains("witness A has type"), "got: {err}");
    }

    /// The `simc` matching this crate's compiler version, if present in the store, so
    /// the out-of-process build uses the same compiler as the in-process ground truth.
    fn provisioned_simc() -> Option<std::path::PathBuf> {
        if let Ok(path) = std::env::var("SIMC_BIN") {
            return Some(path.into());
        }
        let version = simplicityhl::version::SimcDirective::current_version();
        let simc = dirs_home()?.join(".simplex/compilers").join(version).join("bin/simc");
        simc.is_file().then_some(simc)
    }

    fn dirs_home() -> Option<std::path::PathBuf> {
        std::env::var_os("HOME").map(std::path::PathBuf::from)
    }

    // The out-of-process build path: `simc --json` produces the artifact, and it
    // matches the in-process one bit-for-bit (same CMR, same satisfied spend).
    //
    // Skipped unless a matching `simc` is provisioned (`simplex toolchain install
    // <version>`, or point `SIMC_BIN` at one), so the default `cargo test` stays
    // hermetic.
    #[test]
    fn from_json_via_real_simc_matches_from_compiled() {
        let Some(simc) = provisioned_simc() else {
            eprintln!("skipping: no provisioned simc for this compiler version");
            return;
        };

        let src = "fn main() { let a: u16 = witness::A; assert!(jet::eq_16(a, 7)); }";
        let file = std::env::temp_dir().join("smplx_artifact_poc.simf");
        std::fs::write(&file, src).expect("write temp source");

        let out = std::process::Command::new(&simc)
            .arg(&file)
            .arg("--json")
            .output()
            .expect("run simc");
        assert!(
            out.status.success(),
            "simc failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let json = String::from_utf8(out.stdout).expect("utf8 json");

        let from_json = Artifact::from_json(&json).expect("artifact from simc json");
        let from_compiled = Artifact::from_compiled(&compile(src));

        assert_eq!(from_json.cmr(), from_compiled.cmr(), "CMR from simc must match");

        let witness = witness(&[("A", Value::from(UIntValue::U16(7)))]);
        assert_eq!(
            from_json.satisfy(&witness).unwrap().to_vec_with_witness(),
            from_compiled.satisfy(&witness).unwrap().to_vec_with_witness(),
            "spend from the out-of-process artifact must match the in-process one"
        );
    }
}
