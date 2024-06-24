use tracing::debug;

/// Get the command line arguments.
#[tracing::instrument(name = "Runtime arguments")]
pub fn expose_runtime_arguments() {
    let args: Vec<_> = std::env::args().collect();
    debug!("{:?}", args.join(" "));
}
