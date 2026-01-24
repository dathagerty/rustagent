use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub enum AgentMessage {
    // Planning agent messages
    PlanningStarted,
    PlanningResponse(String),
    PlanningToolCall { name: String, args: String },
    PlanningToolResult { name: String, output: String },
    PlanningComplete { spec_path: String },
    PlanningError(String),

    // Execution agent messages
    ExecutionStarted { spec_path: String },
    TaskStarted { task_id: String, title: String },
    TaskResponse(String),
    TaskToolCall { name: String, args: String },
    TaskToolResult { name: String, output: String },
    TaskComplete { task_id: String },
    TaskBlocked { task_id: String, reason: String },
    ExecutionComplete,
    ExecutionError(String),
}

pub type AgentSender = mpsc::Sender<AgentMessage>;
pub type AgentReceiver = mpsc::Receiver<AgentMessage>;

pub fn agent_channel() -> (AgentSender, AgentReceiver) {
    mpsc::channel(100)
}
