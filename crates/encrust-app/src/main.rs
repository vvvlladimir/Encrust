use std::path::PathBuf;

use anyhow::Result;

fn main() -> Result<()> {
    // One optional argument, the model to open on startup. Everything else is done in the
    // window; the `encrust` command line is the place for a real command line.
    let initial_model = std::env::args_os().nth(1).map(PathBuf::from);

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    encrust_app::run(initial_model.as_deref())
}
