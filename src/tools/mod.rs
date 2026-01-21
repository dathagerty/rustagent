use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::llm::ToolDefinition;

/// Trait for tools that can be used by the agent
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns the name of the tool
    fn name(&self) -> &str;

    /// Returns a description of what the tool does
    fn description(&self) -> &str;

    /// Returns the JSON schema for the tool's parameters
    fn parameters(&self) -> serde_json::Value;

    /// Executes the tool with the given parameters
    async fn execute(&self, params: serde_json::Value) -> Result<String>;
}

/// Registry for managing available tools
pub struct ToolRegistry {
    tools: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    /// Creates a new empty tool registry
    pub fn new() -> Self {
        Self {
            tools: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registers a new tool in the registry
    pub fn register(&self, tool: Arc<dyn Tool>) {
        let mut tools = self.tools.write().unwrap();
        tools.insert(tool.name().to_string(), tool);
    }

    /// Gets a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }

    /// Lists all registered tool names
    pub fn list(&self) -> Vec<String> {
        let tools = self.tools.read().unwrap();
        tools.keys().cloned().collect()
    }

    /// Converts all registered tools to LLM tool definitions
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        let tools = self.tools.read().unwrap();
        tools
            .values()
            .map(|tool| ToolDefinition {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.parameters(),
            })
            .collect()
    }
}

impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: Arc::clone(&self.tools),
        }
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub mod factory;
pub mod file;
pub mod permission_check;
pub mod shell;
pub mod signal;
