use crate::tools::Tool;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompletionSignal {
    Complete,
    Blocked,
}

#[derive(Debug, Deserialize)]
struct SignalParams {
    signal: CompletionSignal,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

pub struct SignalTool;

impl SignalTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SignalTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for SignalTool {
    fn name(&self) -> &str {
        "signal_completion"
    }

    fn description(&self) -> &str {
        "Signal task completion or blocked status. Use 'complete' when the task is finished successfully, or 'blocked' if you cannot proceed."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "signal": {
                    "type": "string",
                    "enum": ["complete", "blocked"],
                    "description": "The completion signal: 'complete' for success, 'blocked' if unable to proceed"
                },
                "message": {
                    "type": "string",
                    "description": "Optional message describing what was accomplished (for 'complete')"
                },
                "reason": {
                    "type": "string",
                    "description": "Required reason explaining why the task is blocked (for 'blocked')"
                }
            },
            "required": ["signal"]
        })
    }

    async fn execute(&self, params: serde_json::Value) -> Result<String> {
        let params: SignalParams = serde_json::from_value(params)?;

        match params.signal {
            CompletionSignal::Complete => {
                let msg = params
                    .message
                    .unwrap_or_else(|| "Task completed".to_string());
                Ok(format!("SIGNAL:complete:{}", msg))
            }
            CompletionSignal::Blocked => {
                let reason = params
                    .reason
                    .unwrap_or_else(|| "Unknown reason".to_string());
                Ok(format!("SIGNAL:blocked:{}", reason))
            }
        }
    }
}
