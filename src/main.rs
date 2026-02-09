use rustagent::graph::store::GraphStore;
use rustagent::{config, db, logging, planning, project, ralph};

use clap::{CommandFactory, Parser, Subcommand};
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
    /// View and manage tasks
    Tasks {
        #[command(subcommand)]
        action: Option<TaskAction>,
    },
    /// View and manage decisions
    Decisions {
        #[command(subcommand)]
        action: Option<DecisionAction>,
    },
    /// Show project status
    Status,
    /// Search graph nodes
    Search {
        /// Search query
        query: String,
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

#[derive(Subcommand)]
enum TaskAction {
    /// List all tasks (filterable)
    List {
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        priority: Option<String>,
    },
    /// Show ready tasks
    Ready,
    /// Recommend next task
    Next,
    /// Show task tree
    Tree,
}

#[derive(Subcommand)]
enum DecisionAction {
    /// List active decisions
    List,
    /// Current truth — active decisions only
    Now,
    /// Full evolution including abandoned paths
    History,
    /// Show decision details
    Show { id: String },
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
                    let path_obj = std::path::Path::new(&path);
                    let canonical_path = path_obj.canonicalize()?;
                    let proj = store.add(&name, &canonical_path).await?;
                    println!(
                        "Registered project '{}' ({}) at {}",
                        proj.name,
                        proj.id,
                        proj.path.display()
                    );
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
                ProjectAction::Show { name } => match store.get_by_name(&name).await? {
                    Some(proj) => {
                        println!("Project: {}", proj.name);
                        println!("  ID: {}", proj.id);
                        println!("  Path: {}", proj.path.display());
                        println!("  Registered: {}", proj.registered_at);
                    }
                    None => {
                        println!("Project '{}' not found", name);
                    }
                },
                ProjectAction::Remove { name } => match store.remove(&name).await? {
                    true => {
                        println!("Removed project '{}'", name);
                    }
                    false => {
                        println!("Project '{}' not found", name);
                    }
                },
            }
        }
        Commands::Tasks { action } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let graph_store = rustagent::graph::store::SqliteGraphStore::new(database.clone());

            match action {
                Some(TaskAction::List {
                    status,
                    priority: _,
                }) => {
                    let query = rustagent::graph::store::NodeQuery {
                        node_type: Some(rustagent::graph::NodeType::Task),
                        status: status.and_then(|s| s.parse().ok()),
                        project_id: cli.project.clone(),
                        parent_id: None,
                        query: None,
                    };

                    let tasks = graph_store.query_nodes(&query).await?;
                    if tasks.is_empty() {
                        println!("No tasks found");
                    } else {
                        println!("{:<20} {:<15} {:<30}", "ID", "Status", "Title");
                        println!("{}", "=".repeat(65));
                        for task in tasks {
                            println!("{:<20} {:<15} {:<30}", task.id, task.status, task.title);
                        }
                    }
                }
                Some(TaskAction::Ready) => {
                    if let Some(proj) = cli.project {
                        let tasks = graph_store.get_ready_tasks(&proj).await?;
                        if tasks.is_empty() {
                            println!("No ready tasks");
                        } else {
                            println!("Ready tasks for {}:", proj);
                            println!("{:<20} {:<30}", "ID", "Title");
                            println!("{}", "=".repeat(50));
                            for task in tasks {
                                println!("{:<20} {:<30}", task.id, task.title);
                            }
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                Some(TaskAction::Next) => {
                    if let Some(proj) = cli.project {
                        if let Some(task) = graph_store.get_next_task(&proj).await? {
                            println!("Recommended next task:");
                            println!("  ID: {}", task.id);
                            println!("  Title: {}", task.title);
                            println!("  Description: {}", task.description);
                            if let Some(priority) = task.priority {
                                println!("  Priority: {}", priority);
                            }
                        } else {
                            println!("No ready tasks");
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                Some(TaskAction::Tree) => {
                    if let Some(proj) = cli.project {
                        let subtree = graph_store.get_subtree(&proj).await?;
                        println!("Task tree for {}:", proj);
                        for node in subtree {
                            println!("  - {} ({}): {}", node.id, node.status, node.title);
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                None => {
                    println!("Please specify a task action: list, ready, next, or tree");
                }
            }
        }
        Commands::Decisions { action } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let graph_store = rustagent::graph::store::SqliteGraphStore::new(database.clone());

            match action {
                Some(DecisionAction::List) => {
                    if let Some(proj) = cli.project {
                        let decisions = graph_store.get_active_decisions(&proj).await?;
                        if decisions.is_empty() {
                            println!("No decisions found");
                        } else {
                            println!("Decisions for {}:", proj);
                            println!("{:<20} {:<15} {:<30}", "ID", "Status", "Title");
                            println!("{}", "=".repeat(65));
                            for decision in decisions {
                                println!(
                                    "{:<20} {:<15} {:<30}",
                                    decision.id, decision.status, decision.title
                                );
                            }
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                Some(DecisionAction::Now) => {
                    if let Some(proj) = cli.project {
                        let decisions = graph_store.get_active_decisions(&proj).await?;
                        println!("Current active decisions for {}:", proj);
                        for decision in decisions {
                            println!("  - {}: {}", decision.id, decision.title);
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                Some(DecisionAction::History) => {
                    if let Some(proj) = cli.project {
                        let graph = graph_store.get_full_graph(&proj).await?;
                        println!("Full decision history for {}:", proj);
                        println!("Nodes: {}", graph.nodes.len());
                        println!("Edges: {}", graph.edges.len());
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                Some(DecisionAction::Show { id }) => {
                    if let Some(decision) = graph_store.get_node(&id).await? {
                        println!("Decision: {}", decision.title);
                        println!("  ID: {}", decision.id);
                        println!("  Status: {}", decision.status);
                        println!("  Description: {}", decision.description);
                    } else {
                        println!("Decision '{}' not found", id);
                    }
                }
                None => {
                    println!("Please specify a decision action: list, now, history, or show");
                }
            }
        }
        Commands::Status => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let graph_store = rustagent::graph::store::SqliteGraphStore::new(database.clone());

            if let Some(proj) = cli.project {
                let graph = graph_store.get_full_graph(&proj).await?;
                println!("Status for {}:", proj);
                println!("  Total nodes: {}", graph.nodes.len());
                println!("  Total edges: {}", graph.edges.len());

                println!("\nBreakdown:");
                let mut pending_count = 0;
                let mut ready_count = 0;
                let mut completed_count = 0;
                for node in &graph.nodes {
                    match node.status {
                        rustagent::graph::NodeStatus::Pending => pending_count += 1,
                        rustagent::graph::NodeStatus::Ready => ready_count += 1,
                        rustagent::graph::NodeStatus::Completed => completed_count += 1,
                        _ => {}
                    }
                }
                println!("  Pending: {}", pending_count);
                println!("  Ready: {}", ready_count);
                println!("  Completed: {}", completed_count);
            } else {
                println!("Project must be specified with --project flag");
            }
        }
        Commands::Search { query } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let graph_store = rustagent::graph::store::SqliteGraphStore::new(database.clone());

            let results = graph_store
                .search_nodes(&query, cli.project.as_deref(), None, 50)
                .await?;

            if results.is_empty() {
                println!("No results found for '{}'", query);
            } else {
                println!("Search results for '{}':", query);
                println!("{:<20} {:<15} {:<30}", "ID", "Type", "Title");
                println!("{}", "=".repeat(65));
                for node in results {
                    println!("{:<20} {:<15} {:<30}", node.id, node.node_type, node.title);
                }
            }
        }
    }

    Ok(())
}
