// Keep every distinct argument-parsing failure in this fixture so the public
// diagnostics of `#[simplex::test]` are reviewed together.

#[simplex::test(mock_time =)]
fn malformed_arguments(_: simplex::TestContext) {}

#[simplex::test(unknown)]
fn unknown_argument(_: simplex::TestContext) {}

#[simplex::test(mock_time = 0, mock_time = 1_296_688_602)]
fn duplicate_mock_time(_: simplex::TestContext) {}

#[simplex::test(mock_time = "now")]
fn non_integer_mock_time(_: simplex::TestContext) {}

#[simplex::test(mock_time = 18_446_744_073_709_551_616)]
fn overflowing_mock_time(_: simplex::TestContext) {}

#[simplex::test(mock_time = 1)]
fn mock_time_before_regtest_genesis(_: simplex::TestContext) {}

fn main() {}
