use std::env;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::info;

fn print_version() {
    let version = env!("CARGO_PKG_VERSION");
    let commit = env!("CARGO_PKG_GIT_SHA", "unknown");
    println!("{} {}", version, commit);
}

fn print_usage() {
    println!("Usage: hs5t <config-file>");
    println!();
    println!("Arguments:");
    println!("  <config-file>  Path to the YAML config file (required)");
    println!();
    println!("Options:");
    println!("  --version   Print version and exit");
    println!("  --help      Print this help message and exit");
    println!();
    println!("Environment:");
    println!("  HEV_SOCKS5_TUNNEL_FD   Pre-opened TUN file descriptor (Android/embedded)");
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    // Handle flags before anything else.
    if args.iter().any(|a| a == "--version") {
        print_version();
        std::process::exit(0);
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        std::process::exit(0);
    }

    // Config path is the first positional argument.
    let config_path = match args.get(1) {
        Some(p) => p.clone(),
        None => {
            eprintln!("error: config file path is required");
            eprintln!("Usage: hs5t <config-file>");
            std::process::exit(1);
        }
    };

    // Initialise tracing to stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    // Load and validate config; exit 1 on any error.
    let config = match hs5t_config::Config::from_file(&config_path) {
        Ok(c) => {
            info!("config loaded from '{}'", config_path);
            Arc::new(c)
        }
        Err(e) => {
            eprintln!("error: failed to load config '{}': {}", config_path, e);
            std::process::exit(1);
        }
    };

    let _ = config; // validated; tunnel startup wired up in later loop

    // Set up shared cancellation token for clean shutdown.
    let cancel = CancellationToken::new();

    // Install signal handlers in a background task.
    {
        let cancel_sig = cancel.clone();
        tokio::spawn(async move {
            use tokio::signal::unix::{signal, SignalKind};
            let mut sigterm =
                signal(SignalKind::terminate()).expect("failed to install SIGTERM handler");
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    info!("received SIGINT — shutting down");
                }
                _ = sigterm.recv() => {
                    info!("received SIGTERM — shutting down");
                }
            }
            cancel_sig.cancel();
        });
    }

    // Wait for a shutdown signal.
    cancel.cancelled().await;

    std::process::exit(0);
}
