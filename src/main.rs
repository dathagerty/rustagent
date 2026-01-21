use rustagent::{config, planning, ralph, spec};

use clap::{Parser, Subcommand};
use std::path::PathBuf;

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

/// Find config file in standard locations
fn find_config_path() -> anyhow::Result<PathBuf> {
    // Try current directory
    let local_config = PathBuf::from("rustagent.toml");
    if local_config.exists() {
        return Ok(local_config);
    }

    // Try XDG config directory
    if let Some(config_dir) = dirs::config_dir() {
        let xdg_config = config_dir.join("rustagent").join("config.toml");
        if xdg_config.exists() {
            return Ok(xdg_config);
        }
    }

    // Try home directory
    if let Some(home_dir) = dirs::home_dir() {
        let home_config = home_dir.join(".rustagent").join("config.toml");
        if home_config.exists() {
            return Ok(home_config);
        }
    }

    anyhow::bail!("Config file not found. Please create rustagent.toml in current directory or ~/.rustagent/config.toml")
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
            // Load config from standard locations
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;

            // Use provided spec_dir or default from config
            let dir = spec_dir
                .clone()
                .unwrap_or_else(|| config.rustagent.spec_dir.clone());

            // Create and run planning agent
            let mut agent = planning::PlanningAgent::new(config, dir);
            agent.run().await?;
        }
        Commands::Run { spec_file, max_iterations } => {
            // Load config from standard locations
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;

            // Create and run Ralph loop
            let ralph = ralph::RalphLoop::new(config, spec_file.clone(), *max_iterations);
            ralph.run().await?;
        }
    }

    Ok(())
}
