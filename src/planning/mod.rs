use crate::config::{Config, LlmProvider};
use crate::llm::{LlmClient, Message, ResponseContent, Role};
use crate::llm::anthropic::AnthropicClient;
use crate::tools::{ToolRegistry, file::{ReadFileTool, WriteFileTool, ListFilesTool}, shell::RunCommandTool};
use std::io::{self, Write};
use std::sync::Arc;

/// Planning agent for interactive spec creation
pub struct PlanningAgent {
    client: Box<dyn LlmClient>,
    registry: ToolRegistry,
    pub spec_dir: String,
    conversation: Vec<Message>,
}

impl PlanningAgent {
    /// Create a new planning agent with the given config
    pub fn new(config: Config, spec_dir: String) -> Self {
        // Create LLM client based on provider
        let client: Box<dyn LlmClient> = match config.llm.provider {
            LlmProvider::Anthropic => {
                let api_key = config
                    .anthropic
                    .expect("Anthropic config required for Anthropic provider")
                    .api_key;
                Box::new(AnthropicClient::new(api_key))
            }
            _ => panic!("Unsupported LLM provider"),
        };

        // Create and populate tool registry
        let registry = ToolRegistry::new();
        registry.register(Arc::new(ReadFileTool));
        registry.register(Arc::new(WriteFileTool));
        registry.register(Arc::new(ListFilesTool));
        registry.register(Arc::new(RunCommandTool));

        // Initialize conversation with system message
        let system_message = Message {
            role: Role::System,
            content: PLANNING_SYSTEM_PROMPT.to_string(),
        };

        Self {
            client,
            registry,
            spec_dir,
            conversation: vec![system_message],
        }
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
            self.conversation.push(Message {
                role: Role::User,
                content: input.to_string(),
            });

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

    /// Process a single conversation turn
    async fn process_turn(&mut self) -> anyhow::Result<bool> {
        loop {
            // Get tool definitions
            let tools = self.registry.definitions();

            // Call LLM
            let response = self
                .client
                .chat(self.conversation.clone(), &tools)
                .await
                .map_err(|e| anyhow::anyhow!("LLM error: {}", e))?;

            match response.content {
                ResponseContent::Text(text) => {
                    // Add assistant response to conversation
                    self.conversation.push(Message {
                        role: Role::Assistant,
                        content: text.clone(),
                    });

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
                        self.conversation.push(Message {
                            role: Role::User,
                            content: format!(
                                "Tool result for {}:\n{}",
                                tool_call.name, output
                            ),
                        });
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
