//! Route server diagnostics through libtest's output capture, so a failing test prints the
//! log of what led up to it and a passing test prints nothing.

/// Install the process-wide test subscriber once per suite binary. `RUST_LOG` overrides the
/// server's default filter. Tests that install their own thread-local subscriber (for example
/// `stompymux_rs::logging::Capture`) take precedence on that thread.
pub fn init_logging() {
    let filter = std::env::var("RUST_LOG")
        .ok()
        .filter(|directives| !directives.is_empty())
        .unwrap_or_else(|| stompymux_rs::logging::DEFAULT_FILTER.into());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_ansi(std::io::IsTerminal::is_terminal(&std::io::stdout()))
        .with_test_writer()
        .try_init();
}
