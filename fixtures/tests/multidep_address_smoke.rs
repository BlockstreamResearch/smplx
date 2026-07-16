// A contract importing from two dependencies (one transitive) derives its address
// via the out-of-process pinned compiler; no regtest node needed.
use simplex::provider::SimplicityNetwork;
use simplex_fixtures::artifacts::imports::multidep::MultidepProgram;
use simplex_fixtures::artifacts::imports::multidep::derived_multidep::MultidepArguments;

#[test]
fn multidep_address_via_out_of_process_compiler() {
    let program = MultidepProgram::new(MultidepArguments { prev_hash: 5 });

    // Triggers the out-of-process compile with the embedded `--dep` remappings.
    let script = program.get_script_pubkey(&SimplicityNetwork::default_regtest());
    assert!(!script.is_empty(), "derived a non-empty script pubkey");

    // A different argument must yield a different address (CMR reflects the param).
    let other = MultidepProgram::new(MultidepArguments { prev_hash: 6 });
    let other_script = other.get_script_pubkey(&SimplicityNetwork::default_regtest());
    assert_ne!(
        script, other_script,
        "different params must produce different addresses"
    );
}
