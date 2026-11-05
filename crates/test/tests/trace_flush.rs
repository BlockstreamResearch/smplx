use std::process::{Command, Output};

fn probe(level: &str, scenario: &str) -> Output {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "trace_probe", "--ignored", "--nocapture"])
        .env("SMPLX_TRACE_LEVEL", level)
        .env("SMPLX_TRACE_SCENARIO", scenario)
        .output()
        .unwrap()
}

fn assert_one_trace(output: &Output, succeeds: bool) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.success(), succeeds, "{stderr}");
    assert_eq!(stderr.matches("DBG: jet::eq_32(answer, 42)").count(), 1, "{stderr}");
}

#[test]
fn verbose_regular_tests_flush_successful_execution() {
    for level in ["debug", "trace"] {
        assert_one_trace(&probe(level, "success"), true);
    }
}

#[test]
fn verbose_regular_tests_flush_caught_execution_failures() {
    for level in ["debug", "trace"] {
        assert_one_trace(&probe(level, "failure"), true);
    }
}

#[test]
fn verbose_regular_tests_flush_during_unwinding() {
    for level in ["debug", "trace"] {
        assert_one_trace(&probe(level, "panic"), false);
    }
}

#[test]
fn regular_tests_without_verbosity_stay_quiet() {
    let output = probe("none", "success");
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn already_flushed_execution_is_not_printed_again() {
    assert_one_trace(&probe("trace", "manual"), true);
}

#[test]
fn signer_estimation_only_prints_its_final_execution() {
    let output = probe("trace", "signer");
    assert_one_trace(&output, true);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(stderr.matches("Program info: input=0").count(), 1, "{stderr}");
    assert_eq!(stderr.matches("Eq32(").count(), 1, "{stderr}");
}

// A separate process gives each probe its own process-wide GlobalConfig and
// lets the parent assert the actual stderr, including output during a panic.
#[test]
#[ignore = "subprocess fixture for the trace output tests"]
fn trace_probe() {
    use std::collections::HashMap;

    use simplicityhl::elements::OutPoint;
    use simplicityhl::value::ValueConstructible;
    use simplicityhl::{Arguments, TemplateProgramWitness, Value, WitnessNameToValueMap, WitnessValues};
    use smplx_sdk::global::Verbosity;
    use smplx_sdk::program::logger::ProgramLogger;
    use smplx_sdk::program::{Program, ProgramTrait};
    use smplx_sdk::transaction::{ChangeOutput, FinalTransaction, PartialInput, PartialOutput, ProgramInput, UTXO};
    use smplx_test::TestContext;
    use smplx_test::config::{EsploraConfig, TestConfig};

    let verbosity = match std::env::var("SMPLX_TRACE_LEVEL").unwrap().as_str() {
        "debug" => Verbosity::Debug,
        "trace" => Verbosity::Trace,
        "none" => Verbosity::None,
        level => panic!("unexpected verbosity: {level}"),
    };
    let scenario = std::env::var("SMPLX_TRACE_SCENARIO").unwrap();
    let context = TestContext::from_config(TestConfig {
        verbosity,
        esplora: Some(EsploraConfig {
            // Setup only stores this URL. No probe performs a provider request
            // or starts an Elements/Electrs process.
            url: "http://127.0.0.1:9".into(),
            network: "LiquidTestnet".into(),
        }),
        ..Default::default()
    })
    .unwrap()
    .regular()
    .unwrap();
    let program = Program::new(
        "fn main() { let answer: u32 = witness::ANSWER; assert!(dbg!(jet::eq_32(answer, 42))); }",
        Arguments::default(),
    );
    let succeeds = scenario != "failure" && scenario != "panic";
    let witness = WitnessValues::from_map(HashMap::from([(
        TemplateProgramWitness::witness_from_str("ANSWER"),
        Value::u32(if succeeds { 42 } else { 7 }),
    )]));
    let network = context.get_network();
    let signer = context.get_default_signer();
    let mut transaction = FinalTransaction::new();
    transaction.add_program_input(
        PartialInput::new(UTXO::explicit(
            OutPoint::default(),
            program.get_script_pubkey(network),
            100_000,
            network.policy_asset(),
        )),
        ProgramInput::new(Box::new(program.clone()), witness.clone()),
        None,
    );
    transaction.add_output(PartialOutput::new(
        signer.get_address().script_pubkey(),
        10_000,
        network.policy_asset(),
    ));
    transaction.add_change(ChangeOutput::new(signer.get_address().script_pubkey()));

    if scenario == "signer" {
        // With ample change, estimation signs once with placeholder amounts
        // and once with final amounts. Only the last execution should print.
        signer.finalize_strict(&transaction, 1.0).unwrap();
    } else {
        let (pst, _) = transaction.extract_pst().unwrap();
        assert_eq!(program.execute(&pst, &witness, 0, network).is_ok(), succeeds);
    }
    if scenario == "manual" {
        ProgramLogger::flush_logs();
    }
    assert_ne!(scenario, "panic", "deliberate trace probe panic");
    drop(context);
}
