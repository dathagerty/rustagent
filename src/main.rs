use rustagent::{config, logging, planning, ralph, project, db};

use clap::{Parser, Subcommand, CommandFactory};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "rustagent")]
#[command(about = "A Rust-based AI agent for task execution", long_about = None)]
struct Cli {
    /// Project name (if omitted, resolves from current directory)
    #[arg(long, global = true)]
    project: Option<String>,

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
    /// Manage projects
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
}

#[derive(Subcommand)]
enum ProjectAction {
    /// Register a project
    Add {
        /// Friendly name for the project
        name: String,
        /// Path to the project directory
        path: String,
    },
    /// List all registered projects
    List,
    /// Show project details
    Show {
        /// Project name
        name: String,
    },
    /// Remove a registered project
    Remove {
        /// Project name
        name: String,
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

/// Get the database path in XDG data directory
fn db_path() -> anyhow::Result<PathBuf> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine XDG data directory"))?;
    let db_dir = data_dir.join("rustagent");
    std::fs::create_dir_all(&db_dir)?;
    Ok(db_dir.join("rustagent.db"))
}

/// Resolve project from --project flag or current working directory
#[allow(dead_code)]
async fn resolve_project(
    db: &db::Database,
    project_name: Option<&str>,
) -> anyhow::Result<Option<project::Project>> {
    let store = project::ProjectStore::new(db.clone());

    if let Some(name) = project_name {
        // Look up by name
        store.get_by_name(name).await
    } else {
        // Look up by current working directory
        let cwd = std::env::current_dir()?;
        store.get_by_path(&cwd).await
    }
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
        Commands::Project { action } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let store = project::ProjectStore::new(database);

            match action {
                ProjectAction::Add { name, path } => {
                    let proj = store.add(&name, std::path::Path::new(&path)).await?;
                    println!("Registered project '{}' ({}) at {}",
                             proj.name, proj.id, proj.path.display());
                }
                ProjectAction::List => {
                    let projects = store.list().await?;
                    if projects.is_empty() {
                        println!("No projects registered");
                    } else {
                        println!("{:<20} {:<10} {:<40}", "Name", "ID", "Path");
                        println!("{}", "=".repeat(70));
                        for proj in projects {
                            let path_str = proj.path.display().to_string();
                            let path_display = if path_str.len() > 40 {
                                format!("{}...", &path_str[..37])
                            } else {
                                path_str
                            };
                            println!("{:<20} {:<10} {:<40}", proj.name, proj.id, path_display);
                        }
                    }
                }
                ProjectAction::Show { name } => {
                    match store.get_by_name(&name).await? {
                        Some(proj) => {
                            println!("Project: {}", proj.name);
                            println!("  ID: {}", proj.id);
                            println!("  Path: {}", proj.path.display());
                            println!("  Registered: {}", proj.registered_at);
                        }
                        None => {
                            println!("Project '{}' not found", name);
                        }
                    }
                }
                ProjectAction::Remove { name } => {
                    match store.remove(&name).await? {
                        true => {
                            println!("Removed project '{}'", name);
                        }
                        false => {
                            println!("Project '{}' not found", name);
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
