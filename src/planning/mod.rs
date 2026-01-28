use crate::config::Config;
use crate::llm::factory::create_client;
use crate::llm::{LlmClient, Message, ResponseContent};
use crate::security::{SecurityValidator, permission::CliPermissionHandler};
use crate::tools::ToolRegistry;
use crate::tools::factory::create_default_registry;
use crate::tui::messages::{AgentMessage, AgentSender};
use std::io::{self, Write};
use std::sync::Arc;

/// Planning agent for interactive spec creation
pub struct PlanningAgent {
    client: Arc<dyn LlmClient>,
    registry: ToolRegistry,
    pub spec_dir: String,
    conversation: Vec<Message>,
}

impl PlanningAgent {
    /// Create a new planning agent with the given config
    pub fn new(config: Config, spec_dir: String) -> anyhow::Result<Self> {
        // Get planning-specific LLM config
        let llm_config = config.planning_llm().clone();

        // Create LLM client using factory
        let client = create_client(&config, &llm_config)?;

        // Create security validator and permission handler
        let validator = Arc::new(SecurityValidator::new(config.security.clone())?);
        let permission_handler = Arc::new(CliPermissionHandler);

        // Create and populate tool registry
        let registry = create_default_registry(validator, permission_handler);

        // Initialize conversation with system message
        let system_message = Message::system(PLANNING_SYSTEM_PROMPT);

        Ok(Self {
            client,
            registry,
            spec_dir,
            conversation: vec![system_message],
        })
    }

    /// Run the interactive planning loop
    pub async fn run(&mut self) -> anyhow::Result<()> {
        println!("Planning Agent - Interactive Mode");
        println!("Type your requirements and I'll help you create a detailed spec.");
        println!("Type 'done' when finished or 'exit' to quit.\n");

        loop {
            // Get user input
            print!("You: ");
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;
            let input = input.trim();

            if input == "exit" {
                println!("Exiting planning mode.");
                break;
            }

            if input == "done" {
                println!("Planning complete. Spec saved to {}", self.spec_dir);
                break;
            }

            if input.is_empty() {
                continue;
            }

            // Add user message to conversation
            self.conversation.push(Message::user(input));

            // Process the conversation turn
            match self.process_turn().await {
                Ok(should_continue) => {
                    if !should_continue {
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error: {}", e);
                    // Remove the failed user message
                    self.conversation.pop();
                }
            }
        }

        Ok(())
    }

    /// Run planning with a message sender for TUI integration
    pub async fn run_with_sender(
        &mut self,
        tx: AgentSender,
        initial_message: String,
    ) -> anyhow::Result<()> {
        tx.send(AgentMessage::PlanningStarted).await?;

        self.conversation.push(Message::user(&initial_message));

        loop {
            let tools = self.registry.definitions();
            let response = self.client.chat(self.conversation.clone(), &tools).await?;

            match response.content {
                ResponseContent::Text(text) => {
                    self.conversation.push(Message::assistant(text.clone()));
                    tx.send(AgentMessage::PlanningResponse(text)).await?;

                    // Check for end of turn
                    if matches!(response.stop_reason.as_deref(), Some("end_turn")) {
                        break;
                    }
                    break;
                }
                ResponseContent::ToolCalls(tool_calls) => {
                    for tool_call in &tool_calls {
                        tx.send(AgentMessage::PlanningToolCall {
                            name: tool_call.name.clone(),
                            args: tool_call.parameters.to_string(),
                        })
                        .await?;

                        let tool = self
                            .registry
                            .get(&tool_call.name)
                            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_call.name))?;

                        let result = tool.execute(tool_call.parameters.clone()).await;

                        let output = match result {
                            Ok(output) => output,
                            Err(e) => format!("Error: {}", e),
                        };

                        tx.send(AgentMessage::PlanningToolResult {
                            name: tool_call.name.clone(),
                            output: output.clone(),
                        })
                        .await?;

                        if tool_call.name == "write_file"
                            && let Some(path) =
                                tool_call.parameters.get("path").and_then(|p| p.as_str())
                            && (path.ends_with("spec.json") || path.ends_with(".json"))
                        {
                            tx.send(AgentMessage::PlanningComplete {
                                spec_path: path.to_string(),
                            })
                            .await?;
                        }

                        self.conversation.push(Message::user(format!(
                            "Tool result for {}:\n{}",
                            tool_call.name, output
                        )));
                    }
                    // Continue loop for next LLM response
                }
            }
        }

        Ok(())
    }

    /// Continue conversation with additional user input
    pub async fn continue_with_sender(
        &mut self,
        tx: AgentSender,
        user_message: String,
    ) -> anyhow::Result<()> {
        self.run_with_sender(tx, user_message).await
    }

    /// Process a single conversation turn
    async fn process_turn(&mut self) -> anyhow::Result<bool> {
        loop {
            // Get tool definitions
            let tools = self.registry.definitions();

            // Call LLM
            let response = self.client.chat(self.conversation.clone(), &tools).await?;

            match response.content {
                ResponseContent::Text(text) => {
                    // Add assistant response to conversation
                    self.conversation.push(Message::assistant(text.clone()));

                    // Print assistant response
                    println!("\nAssistant: {}\n", text);

                    // Check for completion
                    if matches!(response.stop_reason.as_deref(), Some("end_turn")) {
                        return Ok(true);
                    }

                    return Ok(true);
                }
                ResponseContent::ToolCalls(tool_calls) => {
                    // Execute each tool call
                    for tool_call in &tool_calls {
                        println!("\n[Executing tool: {}]", tool_call.name);

                        // Get the tool from registry
                        let tool = self
                            .registry
                            .get(&tool_call.name)
                            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_call.name))?;

                        // Execute the tool
                        let result = tool.execute(tool_call.parameters.clone()).await;

                        let output = match result {
                            Ok(output) => {
                                println!("[Tool result: {} bytes]", output.len());
                                output
                            }
                            Err(e) => {
                                println!("[Tool error: {}]", e);
                                format!("Error: {}", e)
                            }
                        };

                        // Add tool result to conversation
                        // Note: Using User role for now as Anthropic expects tool results
                        // in user messages. Future OpenAI provider will use Message::tool_result()
                        self.conversation.push(Message::user(format!(
                            "Tool result for {}:\n{}",
                            tool_call.name, output
                        )));
                    }

                    // Continue the loop to get the next LLM response
                }
            }
        }
    }
}

const PLANNING_SYSTEM_PROMPT: &str = r#"You are a planning agent that helps users create detailed specifications for software development tasks.

Your role is to:
1. Ask clarifying questions to understand the user's requirements
2. Break down complex tasks into smaller, manageable subtasks
3. Define clear acceptance criteria for each task
4. Create a structured specification file in JSON format

When the user provides their requirements, you should:
- Ask about technical constraints, dependencies, and environment
- Identify risks and edge cases
- Suggest best practices and design patterns
- Help define test criteria

Use the available tools to:
- read_file: Read existing files to understand the codebase
- write_file: Create the specification file
- list_files: Explore the project structure
- run_command: Run commands to gather information about the environment

When you have enough information, create a spec.json file with this structure:
{
  "name": "project-name",
  "description": "Brief description of the project",
  "branch_name": "feature/branch-name",
  "created_at": "2024-01-01T00:00:00Z",
  "tasks": [
    {
      "id": "task-1",
      "title": "Task title",
      "description": "Detailed task description",
      "acceptance_criteria": [
        "Criterion 1",
        "Criterion 2"
      ],
      "status": "pending"
    }
  ],
  "learnings": []
}

Be thorough but efficient. Ask questions one at a time to avoid overwhelming the user.
"#;
