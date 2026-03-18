const SLOW_TEST_ENV: &str = "RUN_UI_TESTS";

#[test]
fn ui() {
    if std::env::var(SLOW_TEST_ENV).is_err() {
        eprintln!("Set '{SLOW_TEST_ENV}' to true in order to run a test");
        return;
    }

    let tests = trybuild::TestCases::new();
    tests.pass("tests/ui/*.rs");
}
