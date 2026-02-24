mod server;
mod handlers;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "klock",
    about = "Klock — Coordination protocol for multi-agent systems",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the Klock HTTP coordination server
    Serve {
        /// Port to listen on
        #[arg(short, long, default_value = "3100")]
        port: u16,

        /// Host to bind to
        #[arg(long, default_value = "0.0.0.0")]
        host: String,

        /// Storage backend: "memory" or "sqlite:<path>"
        #[arg(long, default_value = "memory", env = "KLOCK_STORAGE")]
        storage: String,
    },

    /// Check for conflicts from a JSON intent manifest (stdin)
    Check,

    /// Print version information
    Version,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Serve { port, host, storage } => {
            server::run(&host, port, &storage).await;
        }
        Commands::Check => {
            eprintln!("Reading intent manifest from stdin...");
            let mut input = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)
                .expect("Failed to read stdin");

            let manifest: klock_core::state::IntentManifest =
                serde_json::from_str(&input).expect("Invalid JSON manifest");

            let mut client = klock_core::client::KlockClient::new();
            let verdict = client.declare_intent(&manifest);

            println!("{}", serde_json::to_string_pretty(&verdict).unwrap());
        }
        Commands::Version => {
            println!("klock {}", env!("CARGO_PKG_VERSION"));
            println!("Rust coordination kernel for multi-agent systems");
        }
    }
}
