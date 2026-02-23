use std::io;

use tracing::{level_filters::LevelFilter, trace};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

#[test]
fn test_in_custom_folder_custom_333() {
    let _guard = init_logger(); // Store the guard to keep it alive

    if std::env::var(simplex_test::TEST_ENV_NAME).is_err() {
        trace!(
            "Test 'test_in_custom_folder_custom_333' connected with simplex is disabled, run `simplex test` in order to test it"
        );
        println!("Ter to test it");
        return;
    } else {
        tracing::trace!("Running 'test_in_custom_folder_custom_333' with simplex configuration");
        println!("Ter td222o test it");
    }

    assert_eq!(2 + 2, 4);
}

#[test]
fn test_in_custom_folder2_custom_333() {
    assert_eq!(2 + 2, 4);
}

#[derive(Debug)]
pub struct LoggerGuard {
    _std_out_guard: WorkerGuard,
    _std_err_guard: WorkerGuard,
}

pub fn init_logger() -> LoggerGuard {
    let (std_out_writer, std_out_guard) = tracing_appender::non_blocking(io::stdout());
    let (std_err_writer, std_err_guard) = tracing_appender::non_blocking(io::stderr());
    let std_out_layer = fmt::layer()
        .with_writer(std_out_writer)
        .with_ansi(false)
        .with_target(false)
        .with_level(true)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or(EnvFilter::new("debug")));

    let std_err_layer = fmt::layer()
        .with_writer(std_err_writer)
        .with_ansi(false)
        .with_target(false)
        .with_level(true)
        .with_filter(LevelFilter::WARN);

    tracing_subscriber::registry()
        .with(std_out_layer)
        .with(std_err_layer)
        .init();

    trace!("Logging successfully initialized!");
    LoggerGuard {
        _std_out_guard: std_out_guard,
        _std_err_guard: std_err_guard,
    }
}
