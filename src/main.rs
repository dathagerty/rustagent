pub mod config;
pub mod llm;
pub mod spec;
pub mod tools;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "rustagent")]
#[command(about = "A Rust-based AI agent for task execution", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize a new agent specification
    Init {
        /// Directory to store the specification
        #[arg(long)]
        spec_dir: Option<String>,
    },
    /// Create an execution plan from a specification
    Plan {
        /// Directory containing the specification
        #[arg(long)]
        spec_dir: Option<String>,
    },
    /// Execute a plan
    Run {
        /// Path to the specification file
        spec_file: String,
        /// Maximum number of iterations
        #[arg(long)]
        max_iterations: Option<usize>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { spec_dir } => {
            let dir = spec_dir.clone().unwrap_or_else(|| ".".to_string());
            println!("Initializing agent in directory: {}", dir);
            // TODO: Implement initialization logic
        }
        Commands::Plan { spec_dir } => {
            let dir = spec_dir.clone().unwrap_or_else(|| ".".to_string());
            println!("Creating plan from spec directory: {}", dir);
            // TODO: Implement plan creation logic
        }
        Commands::Run { spec_file, max_iterations } => {
            println!("Running agent with spec: {}", spec_file);
            if let Some(max_iter) = max_iterations {
                println!("Maximum iterations: {}", max_iter);
            }
            // TODO: Implement execution logic
        }
    }

    Ok(())
}
