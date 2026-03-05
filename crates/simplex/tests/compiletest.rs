const SLOW_TEST_ENV: &str = "RUN_SLOW_TESTS";
#[test]
fn ui() {
    if let Err(_) = std::env::var(SLOW_TEST_ENV) {
        tracing::trace!("Set '{SLOW_TEST_ENV}' to true in order to run a test");
        return;
    }

    let t = trybuild::TestCases::new();
    t.pass("tests/ui/*.rs");
}
