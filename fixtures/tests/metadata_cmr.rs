use serde_json::Value;

use simplex_fixtures::artifacts::dummy_panic::DummyPanicProgram;
use simplex_fixtures::artifacts::dummy_panic::derived_dummy_panic::DummyPanicArguments;
use simplex_fixtures::artifacts::imports::multidep::MultidepProgram;
use simplex_fixtures::artifacts::imports::multidep::derived_multidep::MultidepArguments;
use simplex_fixtures::artifacts::nested_sig::NestedSigProgram;
use simplex_fixtures::artifacts::nested_sig::derived_nested_sig::NestedSigArguments;
use simplex_fixtures::artifacts::p2pk::P2pkProgram;
use simplex_fixtures::artifacts::p2pk::derived_p2pk::P2pkArguments;
use simplex_fixtures::artifacts::tapleaf_check::TapleafCheckProgram;
use simplex_fixtures::artifacts::tapleaf_check::derived_tapleaf_check::TapleafCheckArguments;

const METADATA_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/artifacts/metadata.json");

fn metadata() -> Value {
    let text = std::fs::read_to_string(METADATA_PATH)
        .unwrap_or_else(|e| panic!("cannot read {METADATA_PATH}, run `simplex build` first: {e}"));

    serde_json::from_str(&text).expect("metadata.json is not valid JSON")
}

/// Lowercase hex, matching the `Display` impl of `Cmr` that produced the metadata entry.
fn to_hex(cmr: [u8; 32]) -> String {
    cmr.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn expected_cmr<'a>(metadata: &'a Value, source: &str) -> &'a str {
    metadata["sources"][source]["cmr"]
        .as_str()
        .unwrap_or_else(|| panic!("metadata.json has no string `cmr` for {source}"))
}

macro_rules! assert_metadata_cmr {
    ($metadata:expr, $source:literal, $program:ty, $arguments:ty) => {{
        let program = <$program>::new(&<$arguments>::default());

        assert_eq!(
            to_hex(program.get_cmr()),
            expected_cmr(&$metadata, $source),
            "CMR in metadata.json does not match the default compilation of {}",
            $source
        );
    }};
}

#[test]
fn metadata_cmrs_match_default_compilation() {
    let metadata = metadata();

    assert_metadata_cmr!(metadata, "p2pk.simf", P2pkProgram, P2pkArguments);
    assert_metadata_cmr!(metadata, "dummy_panic.simf", DummyPanicProgram, DummyPanicArguments);
    assert_metadata_cmr!(metadata, "nested_sig.simf", NestedSigProgram, NestedSigArguments);
    assert_metadata_cmr!(
        metadata,
        "tapleaf_check.simf",
        TapleafCheckProgram,
        TapleafCheckArguments
    );
    assert_metadata_cmr!(metadata, "imports/multidep.simf", MultidepProgram, MultidepArguments);
}

/// The metadata `content` is the flattened source the bindings were generated from, so it must
/// match the `SOURCE` constant `include_simf!` baked into each program.
#[test]
fn metadata_content_matches_included_source() {
    let metadata = metadata();

    for (source, program_source) in [
        ("p2pk.simf", P2pkProgram::SOURCE),
        ("dummy_panic.simf", DummyPanicProgram::SOURCE),
        ("nested_sig.simf", NestedSigProgram::SOURCE),
        ("tapleaf_check.simf", TapleafCheckProgram::SOURCE),
        ("imports/multidep.simf", MultidepProgram::SOURCE),
    ] {
        let recorded = metadata["sources"][source]["content"]
            .as_str()
            .unwrap_or_else(|| panic!("metadata.json has no string `content` for {source}"));

        assert_eq!(recorded, program_source, "metadata.json content is stale for {source}");
    }
}
