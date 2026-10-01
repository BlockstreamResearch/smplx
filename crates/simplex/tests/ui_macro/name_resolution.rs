#![allow(dead_code, unused_macros)]

// Generated paths must not resolve to names defined by the calling crate.
mod simplex {}
mod std {}

struct TestContext;
struct TestConfig;
struct PathBuf;
struct Ok<T>(T);
struct Err<T>(T);
struct Some<T>(T);

macro_rules! panic {
    ($($tokens:tt)*) => {
        compile_error!("the generated test used the caller's panic macro");
    };
}

#[::simplex::test]
fn simple_expansion_uses_qualified_names(_: ::simplex::TestContext) {}

#[::simplex::test(mock_time = 0)]
fn advanced_expansion_uses_qualified_names(_: ::simplex::TestContext) {}

fn main() {}
