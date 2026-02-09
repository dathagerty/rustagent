use rustagent::graph::store::GraphStore;
use rustagent::{config, db, logging, planning, project};

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
    /// Execute a goal with an agent
    Run {
        /// Goal description
        goal: String,
        /// Agent profile to use
        #[arg(long, default_value = "coder")]
        profile: String,
        /// Maximum iterations
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
    /// View sessions and handoff notes
    Sessions {
        #[command(subcommand)]
        action: Option<SessionAction>,
    },
    /// Import/export graph data
    Graph {
        #[command(subcommand)]
        action: GraphAction,
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
    /// Export decisions as ADR markdown files
    Export {
        /// Output directory for markdown files
        #[arg(long)]
        output: Option<String>,
    },
}

#[derive(Subcommand)]
enum SessionAction {
    /// List sessions for current goal
    List {
        /// Goal ID (if omitted, uses current goal context)
        #[arg(long)]
        goal: Option<String>,
    },
    /// Show most recent handoff notes
    Latest {
        /// Goal ID (if omitted, uses current goal context)
        #[arg(long)]
        goal: Option<String>,
    },
}

#[derive(Subcommand)]
enum GraphAction {
    /// Export goals to TOML files
    Export {
        /// Goal ID (if omitted, exports all goals)
        #[arg(long)]
        goal: Option<String>,
        /// Output directory
        #[arg(long)]
        output: Option<String>,
    },
    /// Import TOML files
    Import {
        /// Path to TOML file
        path: String,
        /// Show changes without applying
        #[arg(long)]
        dry_run: bool,
        /// Use file version in conflicts
        #[arg(long)]
        theirs: bool,
        /// Keep database version in conflicts
        #[arg(long)]
        ours: bool,
    },
    /// Diff TOML file against DB
    Diff {
        /// Path to TOML file
        path: String,
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
            goal,
            profile,
            max_iterations,
        } => {
            // Load config from standard locations
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;

            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;

            // Resolve project
            let project_opt = resolve_project(&database, cli.project.as_deref()).await?;
            let project = project_opt.ok_or_else(|| {
                anyhow::anyhow!("No project specified or found in current directory")
            })?;

            // Create goal node in graph
            let graph_store = std::sync::Arc::new(rustagent::graph::store::SqliteGraphStore::new(
                database.clone(),
            ));

            let goal_id = rustagent::graph::generate_goal_id();
            let goal_node = rustagent::graph::GraphNode {
                id: goal_id.clone(),
                project_id: project.id.clone(),
                node_type: rustagent::graph::NodeType::Goal,
                title: goal.clone(),
                description: goal.clone(),
                status: rustagent::graph::NodeStatus::Active,
                priority: None,
                assigned_to: None,
                created_by: None,
                labels: vec!["agent_run".to_string()],
                created_at: chrono::Utc::now(),
                started_at: Some(chrono::Utc::now()),
                completed_at: None,
                blocked_reason: None,
                metadata: std::collections::HashMap::new(),
            };

            graph_store.create_node(&goal_node).await?;
            println!("Created goal: {} ({})", goal, goal_id);

            // Create session
            let session_store = rustagent::graph::session::SessionStore::new(database.clone());
            let session = session_store.create_session(&goal_id, &profile).await?;
            println!("Started session: {}", session.id);

            // Resolve profile
            let resolved_profile =
                rustagent::agent::profile::resolve_profile(&profile, Some(&project.path))?;

            // Build AgentContext
            let agents_md_summaries =
                rustagent::context::resolve_agents_md(&project.path, &[]).unwrap_or_default();

            let ctx = rustagent::agent::AgentContext {
                work_package_tasks: vec![goal_node],
                relevant_decisions: vec![],
                handoff_notes: session.handoff_notes.clone(),
                agents_md_summaries,
                profile: resolved_profile.clone(),
                project_path: project.path.clone(),
                graph_store: graph_store.clone(),
            };

            // Create LLM client
            let llm_client = rustagent::llm::factory::create_client(&config, &config.llm)?;

            // Create tool registry
            let security_validator = std::sync::Arc::new(
                rustagent::security::SecurityValidator::new(config.security.clone())?,
            );
            let permission_handler =
                std::sync::Arc::new(rustagent::security::permission::CliPermissionHandler {});

            let tool_registry = rustagent::tools::factory::create_v2_registry(
                security_validator,
                permission_handler,
                graph_store.clone(),
            );

            // Create AgentRuntime
            let runtime_config = rustagent::agent::runtime::RuntimeConfig {
                max_turns: max_iterations.unwrap_or(100),
                max_consecutive_llm_failures: 3,
                max_consecutive_tool_failures: 3,
                token_budget: resolved_profile.token_budget.unwrap_or(200_000),
                token_budget_warning_pct: 80,
            };

            let runtime = rustagent::agent::runtime::AgentRuntime::new(
                llm_client,
                tool_registry,
                resolved_profile.clone(),
                runtime_config,
            );

            // Run the runtime
            println!("Running agent with profile: {}", profile);
            let outcome = runtime.run(ctx).await?;

            // Handle outcome
            match outcome {
                rustagent::agent::AgentOutcome::Completed { summary } => {
                    println!("Agent completed: {}", summary);
                    graph_store
                        .update_node(
                            &goal_id,
                            Some(rustagent::graph::NodeStatus::Completed),
                            None,
                            None,
                            None,
                        )
                        .await?;
                }
                rustagent::agent::AgentOutcome::Blocked { reason } => {
                    println!("Agent blocked: {}", reason);
                    graph_store
                        .update_node(
                            &goal_id,
                            Some(rustagent::graph::NodeStatus::Blocked),
                            None,
                            Some(&reason),
                            None,
                        )
                        .await?;
                }
                rustagent::agent::AgentOutcome::Failed { error } => {
                    println!("Agent failed: {}", error);
                    graph_store
                        .update_node(
                            &goal_id,
                            Some(rustagent::graph::NodeStatus::Failed),
                            None,
                            Some(&error),
                            None,
                        )
                        .await?;
                }
                rustagent::agent::AgentOutcome::TokenBudgetExhausted {
                    summary,
                    tokens_used,
                } => {
                    println!("Token budget exhausted ({}): {}", tokens_used, summary);
                    graph_store
                        .update_node(
                            &goal_id,
                            Some(rustagent::graph::NodeStatus::Completed),
                            None,
                            Some(&format!(
                                "Token budget exhausted after {} tokens",
                                tokens_used
                            )),
                            None,
                        )
                        .await?;
                }
            }

            // End session
            session_store.end_session(&session.id, &graph_store).await?;
            println!("Session ended");
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
                Some(TaskAction::List { status }) => {
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
                Some(DecisionAction::Export { output }) => {
                    if let Some(proj_id) = cli.project {
                        let output_dir = output.unwrap_or_else(|| ".".to_string());
                        let output_path = std::path::PathBuf::from(&output_dir);

                        match rustagent::graph::export::export_adrs(
                            &graph_store,
                            &proj_id,
                            &output_path,
                        )
                        .await
                        {
                            Ok(files) => {
                                println!("Exported {} decision(s) to {}:", files.len(), output_dir);
                                for file in files {
                                    println!("  {}", file.display());
                                }
                            }
                            Err(e) => {
                                println!("Export failed: {}", e);
                            }
                        }
                    } else {
                        println!("Project must be specified with --project flag");
                    }
                }
                None => {
                    println!(
                        "Please specify a decision action: list, now, history, show, or export"
                    );
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
        Commands::Sessions { action } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let session_store = rustagent::graph::session::SessionStore::new(database.clone());

            match action {
                Some(SessionAction::List { goal }) => {
                    if let Some(goal_id) = goal {
                        match session_store.list_sessions(&goal_id).await {
                            Ok(sessions) => {
                                if sessions.is_empty() {
                                    println!("No sessions found for goal {}", goal_id);
                                } else {
                                    println!("Sessions for {}:", goal_id);
                                    println!("{:<20} {:<25} {:<15}", "ID", "Started", "Status");
                                    println!("{}", "=".repeat(60));
                                    for session in sessions {
                                        let status = if session.ended_at.is_some() {
                                            "Ended"
                                        } else {
                                            "Active"
                                        };
                                        println!(
                                            "{:<20} {:<25} {:<15}",
                                            session.id,
                                            session.started_at.format("%Y-%m-%d %H:%M"),
                                            status
                                        );
                                    }
                                }
                            }
                            Err(e) => println!("Error listing sessions: {}", e),
                        }
                    } else {
                        println!("Goal ID must be specified with --goal flag");
                    }
                }
                Some(SessionAction::Latest { goal }) => {
                    if let Some(goal_id) = goal {
                        match session_store.get_latest_session(&goal_id).await {
                            Ok(Some(session)) => {
                                println!("Latest session for {}:", goal_id);
                                println!("  ID: {}", session.id);
                                println!("  Started: {}", session.started_at);
                                if let Some(ended) = session.ended_at {
                                    println!("  Ended: {}", ended);
                                }
                                if let Some(notes) = session.handoff_notes {
                                    println!("\nHandoff Notes:");
                                    println!("{}", notes);
                                }
                            }
                            Ok(None) => println!("No sessions found for goal {}", goal_id),
                            Err(e) => println!("Error retrieving session: {}", e),
                        }
                    } else {
                        println!("Goal ID must be specified with --goal flag");
                    }
                }
                None => {
                    println!("Please specify a session action: list or latest");
                }
            }
        }
        Commands::Graph { action } => {
            // Open database
            let db_path = db_path()?;
            let database = db::Database::open(&db_path).await?;
            let graph_store = rustagent::graph::store::SqliteGraphStore::new(database.clone());

            match action {
                GraphAction::Export { goal, output } => {
                    if let Some(goal_id) = goal {
                        let project_name =
                            cli.project.clone().unwrap_or_else(|| "unknown".to_string());
                        match rustagent::graph::interchange::export_goal(
                            &graph_store,
                            &goal_id,
                            &project_name,
                        )
                        .await
                        {
                            Ok(toml_content) => {
                                if let Some(output_path) = output {
                                    // Write to file
                                    match std::fs::write(&output_path, &toml_content) {
                                        Ok(_) => println!("Exported goal to {}", output_path),
                                        Err(e) => println!("Failed to write file: {}", e),
                                    }
                                } else {
                                    // Print to stdout
                                    println!("{}", toml_content);
                                }
                            }
                            Err(e) => println!("Export failed: {}", e),
                        }
                    } else {
                        println!("Goal ID must be specified with --goal flag");
                    }
                }
                GraphAction::Import {
                    path,
                    dry_run,
                    theirs,
                    ours,
                } => match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        let strategy = if theirs {
                            rustagent::graph::interchange::ImportStrategy::Theirs
                        } else if ours {
                            rustagent::graph::interchange::ImportStrategy::Ours
                        } else {
                            rustagent::graph::interchange::ImportStrategy::Merge
                        };

                        if dry_run {
                            // Parse the TOML and show what would be imported without writing
                            match toml::from_str::<rustagent::graph::interchange::GoalFile>(
                                &content,
                            ) {
                                Ok(goal_file) => {
                                    println!("[DRY RUN] Changes that would be applied:");
                                    println!("  Nodes to process: {}", goal_file.nodes.len());
                                    println!("  Edges to process: {}", goal_file.edges.len());
                                    println!("  Import strategy: {:?}", strategy);
                                }
                                Err(e) => println!("Failed to parse TOML: {}", e),
                            }
                        } else {
                            match rustagent::graph::interchange::import_goal(
                                &graph_store,
                                &content,
                                strategy,
                            )
                            .await
                            {
                                Ok(result) => {
                                    println!("  Added nodes: {}", result.added_nodes);
                                    println!("  Added edges: {}", result.added_edges);
                                    println!("  Unchanged: {}", result.unchanged);
                                    if !result.conflicts.is_empty() {
                                        println!("  Conflicts: {}", result.conflicts.len());
                                    }
                                    if !result.skipped_edges.is_empty() {
                                        println!("  Skipped edges: {}", result.skipped_edges.len());
                                    }
                                }
                                Err(e) => println!("Import failed: {}", e),
                            }
                        }
                    }
                    Err(e) => println!("Failed to read file: {}", e),
                },
                GraphAction::Diff { path } => match std::fs::read_to_string(&path) {
                    Ok(content) => {
                        match rustagent::graph::interchange::diff_goal(&graph_store, &content).await
                        {
                            Ok(result) => {
                                println!("Diff results for {}:", path);
                                if !result.added_nodes.is_empty() {
                                    println!("  Added nodes: {}", result.added_nodes.len());
                                    for node_id in &result.added_nodes {
                                        println!("    + {}", node_id);
                                    }
                                }
                                if !result.changed_nodes.is_empty() {
                                    println!("  Changed nodes: {}", result.changed_nodes.len());
                                    for (node_id, fields) in &result.changed_nodes {
                                        println!("    ~ {} ({})", node_id, fields.join(", "));
                                    }
                                }
                                if !result.removed_nodes.is_empty() {
                                    println!("  Removed nodes: {}", result.removed_nodes.len());
                                    for node_id in &result.removed_nodes {
                                        println!("    - {}", node_id);
                                    }
                                }
                                if !result.added_edges.is_empty() {
                                    println!("  Added edges: {}", result.added_edges.len());
                                }
                                if !result.removed_edges.is_empty() {
                                    println!("  Removed edges: {}", result.removed_edges.len());
                                }
                                println!("  Unchanged nodes: {}", result.unchanged_nodes);
                                println!("  Unchanged edges: {}", result.unchanged_edges);
                            }
                            Err(e) => println!("Diff failed: {}", e),
                        }
                    }
                    Err(e) => println!("Failed to read file: {}", e),
                },
            }
        }
    }

    Ok(())
}
