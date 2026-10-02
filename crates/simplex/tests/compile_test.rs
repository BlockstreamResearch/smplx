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

#[test]
fn ui_macro_test() {
    if std::env::var(SLOW_TEST_ENV).is_err() {
        eprintln!("Set '{SLOW_TEST_ENV}' to true in order to run a test");
        return;
    }

    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui_macro/fail.rs");
    tests.pass("tests/ui_macro/name_resolution.rs");
}
