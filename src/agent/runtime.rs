use crate::agent::{AgentContext, AgentId, AgentOutcome, AgentProfile};
use crate::context::{ContextBudget, ContextBuilder};
use crate::llm::{LlmClient, Message, ResponseContent};
use crate::message::{MessageBus, WorkerMessage};
use crate::tools::ToolRegistry;
use anyhow::Result;
use std::sync::Arc;

/// Configuration for the AgentRuntime
#[derive(Clone)]
pub struct RuntimeConfig {
    /// Maximum number of turns to run (default: 100)
    pub max_turns: usize,
    /// Maximum consecutive LLM failures before blocking (default: 3)
    pub max_consecutive_llm_failures: usize,
    /// Maximum consecutive tool failures before blocking (default: 3)
    pub max_consecutive_tool_failures: usize,
    /// Token budget for this run (default: 200_000)
    pub token_budget: usize,
    /// Warning threshold as percentage of budget (default: 80)
    pub token_budget_warning_pct: u8,
    /// Optional message bus for check-in reports (None for single-agent mode)
    pub message_bus: Option<Arc<dyn MessageBus>>,
    /// Agent ID for check-in reports (None for single-agent mode)
    pub agent_id: Option<AgentId>,
    /// Check-in interval in turns (default: 10)
    pub check_in_interval: usize,
}

impl std::fmt::Debug for RuntimeConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeConfig")
            .field("max_turns", &self.max_turns)
            .field(
                "max_consecutive_llm_failures",
                &self.max_consecutive_llm_failures,
            )
            .field(
                "max_consecutive_tool_failures",
                &self.max_consecutive_tool_failures,
            )
            .field("token_budget", &self.token_budget)
            .field("token_budget_warning_pct", &self.token_budget_warning_pct)
            .field("message_bus", &self.message_bus.is_some())
            .field("agent_id", &self.agent_id)
            .field("check_in_interval", &self.check_in_interval)
            .finish()
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            max_turns: 100,
            max_consecutive_llm_failures: 3,
            max_consecutive_tool_failures: 3,
            token_budget: 200_000,
            token_budget_warning_pct: 80,
            message_bus: None,
            agent_id: None,
            check_in_interval: 10,
        }
    }
}

/// The agentic loop: LLM call -> tool execution -> repeat
pub struct AgentRuntime {
    client: Arc<dyn LlmClient>,
    tools: ToolRegistry,
    #[allow(dead_code)]
    profile: AgentProfile,
    config: RuntimeConfig,
}

impl AgentRuntime {
    /// Create a new AgentRuntime
    pub fn new(
        client: Arc<dyn LlmClient>,
        tools: ToolRegistry,
        profile: AgentProfile,
        config: RuntimeConfig,
    ) -> Self {
        Self {
            client,
            tools,
            profile,
            config,
        }
    }

    /// Run the agentic loop
    pub async fn run(&self, ctx: AgentContext) -> Result<AgentOutcome> {
        let budget = ContextBudget::default();
        let system_prompt = ContextBuilder::build_system_prompt_with_budget(&ctx, &budget);
        let mut messages = vec![Message::system(system_prompt)];
        let mut cumulative_tokens: usize = 0;
        let mut warned_about_budget = false;
        let mut consecutive_llm_failures = 0;
        let mut consecutive_tool_failures = 0;
        let mut turn = 0;

        loop {
            // Check turn limit
            if turn >= self.config.max_turns {
                return Ok(AgentOutcome::Completed {
                    summary: format!("Turn limit reached after {} turns", self.config.max_turns),
                    tokens_used: cumulative_tokens,
                });
            }
            turn += 1;

            // Send check-in progress report if configured
            if let (Some(bus), Some(id)) = (&self.config.message_bus, &self.config.agent_id)
                && turn > 1
                && (turn - 1) % self.config.check_in_interval == 0
            {
                let _ = bus
                    .send(
                        &"orchestrator".to_string(),
                        WorkerMessage::ProgressReport {
                            agent_id: id.clone(),
                            turn: turn - 1,
                            summary: format!("Turn {}: processing", turn - 1),
                        },
                    )
                    .await;
            }

            // Check token budget warning threshold
            let token_warning_threshold =
                (self.config.token_budget * self.config.token_budget_warning_pct as usize) / 100;
            if cumulative_tokens >= token_warning_threshold && !warned_about_budget {
                warned_about_budget = true;
                messages.push(Message::system(
                    "You are approaching your token budget. Wrap up your current work and signal completion.".to_string()
                ));
            }

            // Check token budget exhausted
            if cumulative_tokens >= self.config.token_budget {
                return Ok(AgentOutcome::TokenBudgetExhausted {
                    summary: "Token budget exhausted".to_string(),
                    tokens_used: cumulative_tokens,
                });
            }

            // Call LLM
            let tool_definitions = self.tools.definitions();
            let response = match self.client.chat(messages.clone(), &tool_definitions).await {
                Ok(resp) => {
                    consecutive_llm_failures = 0;
                    resp
                }
                Err(e) => {
                    consecutive_llm_failures += 1;
                    if consecutive_llm_failures >= self.config.max_consecutive_llm_failures {
                        return Ok(AgentOutcome::Blocked {
                            reason: format!(
                                "LLM failures: {} consecutive failures ({:?})",
                                self.config.max_consecutive_llm_failures, e
                            ),
                        });
                    }
                    // Send error back to LLM for self-correction
                    messages.push(Message::assistant(format!("Error: {}", e)));
                    continue;
                }
            };

            // Track token usage
            if let Some(input_tokens) = response.input_tokens {
                cumulative_tokens += input_tokens;
            }
            if let Some(output_tokens) = response.output_tokens {
                cumulative_tokens += output_tokens;
            }

            // Process response content
            match response.content {
                ResponseContent::Text(text) => {
                    messages.push(Message::assistant(text));
                }
                ResponseContent::ToolCalls(tool_calls) => {
                    // Add the assistant's tool calls to the message history
                    let tool_calls_json = serde_json::to_string(&tool_calls)?;
                    messages.push(Message::assistant(tool_calls_json));

                    // Execute each tool
                    for tool_call in tool_calls {
                        // Check for signal_completion
                        if tool_call.name == "signal_completion" {
                            if let Some(tool) = self.tools.get(&tool_call.name) {
                                match tool.execute(tool_call.parameters).await {
                                    Ok(output) => {
                                        if output.contains("SIGNAL:complete") {
                                            // Extract message from output
                                            let message = output
                                                .strip_prefix("SIGNAL:complete:")
                                                .unwrap_or("Task completed")
                                                .to_string();
                                            return Ok(AgentOutcome::Completed {
                                                summary: message,
                                                tokens_used: cumulative_tokens,
                                            });
                                        } else if output.contains("SIGNAL:blocked") {
                                            let reason = output
                                                .strip_prefix("SIGNAL:blocked:")
                                                .unwrap_or("Task blocked")
                                                .to_string();
                                            return Ok(AgentOutcome::Blocked { reason });
                                        }
                                    }
                                    Err(e) => {
                                        consecutive_tool_failures += 1;
                                        let error_msg = format!("Tool execution failed: {}", e);
                                        messages.push(Message::tool_result(
                                            tool_call.id.clone(),
                                            error_msg,
                                        ));
                                    }
                                }
                            }
                            continue;
                        }

                        // Execute regular tool
                        match self.tools.get(&tool_call.name) {
                            Some(tool) => match tool.execute(tool_call.parameters).await {
                                Ok(output) => {
                                    consecutive_tool_failures = 0;
                                    messages.push(Message::tool_result(tool_call.id, output));
                                }
                                Err(e) => {
                                    consecutive_tool_failures += 1;
                                    if consecutive_tool_failures
                                        >= self.config.max_consecutive_tool_failures
                                    {
                                        return Ok(AgentOutcome::Blocked {
                                            reason: format!(
                                                "Tool failures: {} consecutive failures",
                                                self.config.max_consecutive_tool_failures
                                            ),
                                        });
                                    }
                                    let error_msg = format!("Tool error: {}", e);
                                    messages.push(Message::tool_result(tool_call.id, error_msg));
                                }
                            },
                            None => {
                                consecutive_tool_failures += 1;
                                if consecutive_tool_failures
                                    >= self.config.max_consecutive_tool_failures
                                {
                                    return Ok(AgentOutcome::Blocked {
                                        reason: format!(
                                            "Tool failures: {} consecutive failures",
                                            self.config.max_consecutive_tool_failures
                                        ),
                                    });
                                }
                                let error_msg = format!("Unknown tool: {}", tool_call.name);
                                messages.push(Message::tool_result(tool_call.id, error_msg));
                            }
                        }
                    }
                }
            }
        }
    }
}
