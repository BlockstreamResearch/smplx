use simplex_test::TestContextBuilder;
use std::path::PathBuf;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[test]
fn test_in_custom_folder_custom_333() {
    let user_context = |_| -> _ {
        assert_eq!(2 + 2, 4);
    };
    let test_context = match std::env::var("SIMPLEX_TEST_ENV") {
        Err(e) => {
            tracing::trace!(
                "Test 'test_in_custom_folder_custom_333' connected with simplex is disabled, run `simplex test` in order to test it, err: '{e}'"
            );
            return;
        }
        Ok(path) => {
            let path = PathBuf::from(path);
            let test_context = TestContextBuilder::FromConfigPath(path).build().unwrap();
            test_context
        }
    };
    tracing::trace!("Running 'test_in_custom_folder_custom_333' with simplex configuration");
    user_context(test_context)
}

#[test]
fn test_in_custom_folder2_custom_333() {
    assert_eq!(2 + 2, 4);
}
