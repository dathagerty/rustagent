use crate::config::Config;
use crate::llm::factory::create_client;
use crate::llm::{LlmClient, Message, ResponseContent};
use crate::security::{SecurityValidator, permission::CliPermissionHandler};
use crate::spec::{Spec, TaskStatus};
use crate::tools::ToolRegistry;
use crate::tools::factory::create_default_registry;
use anyhow::{Context, Result};
use chrono::Utc;
use std::sync::Arc;

const DEFAULT_MAX_ITERATIONS: usize = 100;

pub struct RalphLoop {
    client: Arc<dyn LlmClient>,
    tools: ToolRegistry,
    pub spec_path: String,
    max_iterations: usize,
}

impl RalphLoop {
    pub fn new(config: Config, spec_path: String, max_iterations: Option<usize>) -> Result<Self> {
        // Get ralph-specific LLM config
        let llm_config = config.ralph_llm().clone();

        // Create LLM client using factory
        let client = create_client(&config, &llm_config)?;

        // Create security validator and permission handler
        let validator = Arc::new(SecurityValidator::new(config.security.clone())?);
        let permission_handler = Arc::new(CliPermissionHandler);

        // Register tools
        let tools = create_default_registry(validator, permission_handler);

        let max_iterations = max_iterations
            .or(config.rustagent.max_iterations)
            .unwrap_or(DEFAULT_MAX_ITERATIONS);

        Ok(Self {
            client,
            tools,
            spec_path,
            max_iterations,
        })
    }

    pub async fn run(&self) -> Result<()> {
        println!("Starting Ralph Loop");
        println!("Max iterations: {}", self.max_iterations);
        println!();

        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                println!("Reached max iterations ({})", self.max_iterations);
                break;
            }

            println!("=== Iteration {} ===", iteration);

            // Load spec fresh each iteration
            let mut spec = Spec::load(&self.spec_path).context("Failed to load spec")?;

            // Find next pending task
            let task = match spec.find_next_task() {
                Some(t) => t.clone(),
                None => {
                    println!("No more pending tasks. All tasks complete!");
                    break;
                }
            };

            println!("Executing task: {} - {}", task.id, task.title);

            // Mark task as in progress
            {
                let task_mut = spec
                    .find_task_mut(&task.id)
                    .context("Task not found in spec")?;
                task_mut.status = TaskStatus::InProgress;
            }
            spec.save(&self.spec_path).context("Failed to save spec")?;

            // Execute the task
            match self.execute_task(&task.id).await {
                Ok(signal) => {
                    // Reload spec to get any changes made during execution
                    let mut spec = Spec::load(&self.spec_path)?;

                    match signal.as_str() {
                        "TASK_COMPLETE" => {
                            println!("Task completed successfully");
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found in spec")?;
                            task_mut.status = TaskStatus::Complete;
                            task_mut.completed_at = Some(Utc::now());
                            spec.save(&self.spec_path)?;
                        }
                        "TASK_BLOCKED" => {
                            println!("Task is blocked");
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found in spec")?;
                            task_mut.status = TaskStatus::Blocked;
                            spec.save(&self.spec_path)?;
                        }
                        _ => {
                            println!("Unknown signal: {}", signal);
                            // Reset to pending to retry
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found in spec")?;
                            task_mut.status = TaskStatus::Pending;
                            spec.save(&self.spec_path)?;
                        }
                    }
                }
                Err(e) => {
                    println!("Error executing task: {}", e);
                    // Reset to pending to retry
                    let mut spec = Spec::load(&self.spec_path)?;
                    let task_mut = spec
                        .find_task_mut(&task.id)
                        .context("Task not found in spec")?;
                    task_mut.status = TaskStatus::Pending;
                    spec.save(&self.spec_path)?;
                    break;
                }
            }

            println!();
        }

        println!("Ralph Loop finished");
        Ok(())
    }

    async fn execute_task(&self, task_id: &str) -> Result<String> {
        let context = self.build_context(task_id)?;
        let tool_definitions = self.tools.definitions();

        let mut messages = vec![Message::user(context)];

        // Agentic loop - continue until we get a completion signal
        let max_turns = 50;
        for turn in 0..max_turns {
            println!("  Turn {}", turn + 1);

            let response = self
                .client
                .chat(messages.clone(), &tool_definitions)
                .await?;

            match response.content {
                ResponseContent::Text(text) => {
                    println!("  Response: {}", text);

                    // Check for completion signals
                    if text.contains("TASK_COMPLETE") {
                        return Ok("TASK_COMPLETE".to_string());
                    }
                    if text.contains("TASK_BLOCKED") {
                        return Ok("TASK_BLOCKED".to_string());
                    }

                    // Add assistant response to conversation
                    messages.push(Message::assistant(text));
                }
                ResponseContent::ToolCalls(tool_calls) => {
                    println!("  Executing {} tool calls", tool_calls.len());

                    // Execute all tool calls
                    let mut results = Vec::new();
                    for tool_call in tool_calls {
                        println!("    Tool: {}", tool_call.name);

                        let tool = self.tools.get(&tool_call.name).context("Tool not found")?;

                        match tool.execute(tool_call.parameters).await {
                            Ok(output) => {
                                results
                                    .push(format!("Tool: {}\nResult: {}", tool_call.name, output));
                            }
                            Err(e) => {
                                results.push(format!("Tool: {}\nError: {}", tool_call.name, e));
                            }
                        }
                    }

                    // Add tool results as user message
                    // Note: Using User role for now as Anthropic expects tool results
                    // in user messages. Future OpenAI provider will use Message::tool_result()
                    let results_text = results.join("\n\n");
                    messages.push(Message::user(results_text));
                }
            }
        }

        Err(anyhow::anyhow!("Reached max turns without completion"))
    }

    fn build_context(&self, task_id: &str) -> Result<String> {
        let spec = Spec::load(&self.spec_path)?;

        let task = spec
            .tasks
            .iter()
            .find(|t| t.id == task_id)
            .context("Task not found")?;

        let mut context = String::new();

        context.push_str("You are Ralph, an autonomous coding agent.\n\n");
        context.push_str("PROJECT CONTEXT:\n");
        context.push_str(&format!("Spec: {}\n", spec.name));
        context.push_str(&format!("Description: {}\n", spec.description));
        context.push_str(&format!("Branch: {}\n\n", spec.branch_name));

        context.push_str("YOUR TASK:\n");
        context.push_str(&format!("ID: {}\n", task.id));
        context.push_str(&format!("Title: {}\n", task.title));
        context.push_str(&format!("Description: {}\n\n", task.description));

        if !task.acceptance_criteria.is_empty() {
            context.push_str("ACCEPTANCE CRITERIA:\n");
            for criterion in &task.acceptance_criteria {
                context.push_str(&format!("- {}\n", criterion));
            }
            context.push('\n');
        }

        if !spec.learnings.is_empty() {
            context.push_str("LEARNINGS FROM PREVIOUS TASKS:\n");
            for learning in &spec.learnings {
                context.push_str(&format!("- {}\n", learning));
            }
            context.push('\n');
        }

        context.push_str("INSTRUCTIONS:\n");
        context.push_str("1. Execute the task using available tools\n");
        context.push_str("2. Use read_file to examine code\n");
        context.push_str("3. Use write_file to create/modify files\n");
        context.push_str("4. Use shell_command to run tests, builds, git commands\n");
        context.push_str("5. When complete, respond with TASK_COMPLETE\n");
        context.push_str("6. If blocked, explain why and respond with TASK_BLOCKED\n");
        context.push('\n');
        context.push_str("Begin executing the task now.\n");

        Ok(context)
    }
}
