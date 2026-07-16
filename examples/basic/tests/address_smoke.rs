// A parametric contract derives its address via the out-of-process pinned
// compiler; no regtest node needed.
use simplex::program::Program;
use simplex::provider::SimplicityNetwork;
use simplex_example::artifacts::p2pk::P2pkProgram;
use simplex_example::artifacts::p2pk::derived_p2pk::{P2pkArguments, P2pkWitness};

#[test]
fn p2pk_address_via_out_of_process_compiler() {
    let arguments = P2pkArguments { public_key: [2u8; 32] };
    let program = P2pkProgram::new(arguments);

    // Triggers the out-of-process compile.
    let script = program.get_script_pubkey(&SimplicityNetwork::default_regtest());
    assert!(!script.is_empty(), "derived a non-empty script pubkey");

    // A different argument must yield a different address (CMR reflects the param).
    let other = P2pkProgram::new(P2pkArguments { public_key: [3u8; 32] });
    let other_script = other.get_script_pubkey(&SimplicityNetwork::default_regtest());
    assert_ne!(
        script, other_script,
        "different params must produce different addresses"
    );

    // The ABI accessors work from the baked metadata, no compiler needed.
    let _witness = P2pkWitness::default();
    let types = AsRef::<Program>::as_ref(&program)
        .get_witness_types()
        .expect("witness types from baked ABI");
    assert!(
        types
            .get(&simplex::simplicityhl::str::WitnessName::from_str_unchecked(
                "SIGNATURE"
            ))
            .is_some(),
        "SIGNATURE witness type is present in the baked ABI"
    );
}
