//! Source-free program artifact: what a [`Program`](super::core::Program) derives
//! its address from and satisfies a spend with, produced by the pinned
//! out-of-process `simc` rather than the linked frontend.

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
    /// Witness names in DAG post order, the order [`Self::satisfy`] assigns values in.
    witness_names: Vec<WitnessName>,
    /// CMR-keyed debug symbols. Instrumentation is part of the program, so the
    /// symbols describe exactly this artifact.
    debug_symbols: Option<DebugSymbols>,
}

impl Artifact {
    /// Freezes a compiled program into a source-free artifact.
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

    /// Builds an artifact from `simc --json` output. The CMR is recomputed from the
    /// program bytes and cross-checked against the declared one, so a corrupt output
    /// fails here rather than as a wrong address.
    ///
    /// # Errors
    /// Returns an error if the JSON is malformed, the program bytes fail to decode,
    /// or the CMRs disagree.
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

    /// The program's CMR — the address and control block derive from this alone.
    #[must_use]
    pub fn cmr(&self) -> Cmr {
        self.cmr
    }

    /// The program's debug symbols, when it was compiled with debug instrumentation.
    #[must_use]
    pub fn debug_symbols(&self) -> Option<&DebugSymbols> {
        self.debug_symbols.as_ref()
    }

    /// Reconstructs the (unpruned) redeem program from the frozen commit DAG,
    /// assigning witness values by post-order index instead of by name.
    ///
    /// # Errors
    /// Returns an error if a witness value is missing or wrong-typed, or the witness
    /// node count disagrees with the frozen layout.
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

    #[test]
    fn artifact_reports_missing_witness() {
        let compiled = compile("fn main() { let a: u16 = witness::A; assert!(jet::eq_16(a, 7)); }");
        let artifact = Artifact::from_compiled(&compiled);
        let err = artifact.satisfy(&witness(&[])).unwrap_err();
        assert!(err.contains("missing witness"), "got: {err}");
    }

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

    // Skipped unless a matching `simc` is provisioned (`simplex toolchain install
    // <version>`, or `SIMC_BIN`), so the default `cargo test` stays hermetic.
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

    // Skipped unless a matching `simc` is provisioned, like the test above.
    #[test]
    fn compile_with_dependency_via_real_simc() {
        let Some(simc) = provisioned_simc() else {
            eprintln!("skipping: no provisioned simc for this compiler version");
            return;
        };

        let sources = [
            (
                "main.simf".to_string(),
                "use math::simple_op::hash;\n\
                 fn main() { let a: u32 = witness::A; assert!(jet::eq_32(hash(a, 5), 3)); }"
                    .to_string(),
            ),
            (
                "__deps__/0/simple_op.simf".to_string(),
                "pub fn hash(x: u32, y: u32) -> u32 { jet::xor_32(x, y) }".to_string(),
            ),
        ];
        let deps = [(String::new(), "math".to_string(), "__deps__/0".to_string())];

        let artifact = crate::compiler::compile(
            &simc,
            &sources,
            "main.simf",
            &deps,
            &simplicityhl::Arguments::default(),
            false,
        )
        .expect("dep-using program compiles out of process");

        let witness = witness(&[("A", Value::from(UIntValue::U32(6)))]);
        artifact.satisfy(&witness).expect("dep-using artifact satisfies");
    }

    // The `-v`/`-vv` path end to end, without a node. Skipped unless a matching
    // `simc` is provisioned, like the tests above.
    #[test]
    fn debug_compile_carries_symbols_and_traces() {
        use simplicityhl::tracker::TrackerLogLevel;

        use crate::program::logger::ProgramLogger;

        let Some(simc) = provisioned_simc() else {
            eprintln!("skipping: no provisioned simc for this compiler version");
            return;
        };

        let sources = [(
            "main.simf".to_string(),
            "fn main() { let x: u32 = dbg!(witness::A); assert!(jet::is_zero_32(x)); }".to_string(),
        )];
        let args = simplicityhl::Arguments::default();

        let plain = crate::compiler::compile(&simc, &sources, "main.simf", &[], &args, false).expect("plain compile");
        assert!(plain.debug_symbols().is_none(), "plain compile must carry no symbols");

        let debug = crate::compiler::compile(&simc, &sources, "main.simf", &[], &args, true).expect("debug compile");
        let symbols = debug.debug_symbols().expect("debug compile must carry symbols");
        assert_ne!(
            plain.cmr(),
            debug.cmr(),
            "debug instrumentation is part of the program, so the CMR must differ"
        );

        let witness = witness(&[("A", Value::from(UIntValue::U32(0)))]);
        let redeem = debug.satisfy(&witness).expect("debug artifact satisfies");

        let mut tracker = ProgramLogger::make_tracker(symbols, TrackerLogLevel::Trace);
        let env = simplicityhl::dummy_env::dummy();
        let _pruned = redeem
            .prune_with_tracker(&env, &mut tracker)
            .expect("prune with tracker");

        let trace = ProgramLogger::take_trace_buffer();
        assert!(
            trace.iter().any(|line| line.contains("DBG: witness::A")),
            "expected the dbg! expression in the trace, got: {trace:?}"
        );
    }
}
