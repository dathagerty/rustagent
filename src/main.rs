use rustagent::{config, logging, planning, ralph};

use clap::{Parser, Subcommand, CommandFactory};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rustagent")]
#[command(about = "A Rust-based AI agent for task execution", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
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

    anyhow::bail!(
        "Config file not found. Please create rustagent.toml in current directory or ~/.rustagent/config.toml"
    )
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = logging::init_logging()?;

    let cli = Cli::parse();

    // If no command specified, print help
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        return Ok(());
    };

    match command {
        Commands::Init { spec_dir } => {
            let dir = spec_dir.clone().unwrap_or_else(|| "specs".to_string());

            let spec_path = std::path::Path::new(&dir);
            if !spec_path.exists() {
                std::fs::create_dir_all(spec_path)?;
                println!("Created spec directory: {}", dir);
            } else {
                println!("Spec directory already exists: {}", dir);
            }

            let config_path = std::path::Path::new("rustagent.toml");
            if !config_path.exists() {
                let template = include_str!("../rustagent.toml.example");
                std::fs::write(config_path, template)?;
                println!("Created config template: rustagent.toml");
                println!("Please edit rustagent.toml to add your API keys.");
            } else {
                println!("Config file already exists: rustagent.toml");
            }

            println!("\nInitialization complete!");
            println!("Next steps:");
            println!("  1. Edit rustagent.toml with your API keys");
            println!("  2. Run 'rustagent plan' to create a spec");
            println!("  3. Run 'rustagent run <spec-file>' to execute");
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
            let mut agent = planning::PlanningAgent::new(config, dir)?;
            agent.run().await?;
        }
        Commands::Run {
            spec_file,
            max_iterations,
        } => {
            // Load config from standard locations
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;

            // Create and run Ralph loop
            let ralph = ralph::RalphLoop::new(config, spec_file.clone(), max_iterations)?;
            ralph.run().await?;
        }
    }

    Ok(())
}
