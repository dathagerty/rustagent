# TUI Phase 2 Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add async agent integration and polish features to the TUI.

**Architecture:** TUI spawns agents as tokio tasks, receives structured messages via mpsc channels, routes to view state. Event loop uses `tokio::select!` to multiplex terminal events and agent messages.

**Tech Stack:** tokio (channels, select, spawn), ratatui, crossterm (existing)

---

## Phase 1: Agent Integration

### Task 1: Add Message Types

**Files:**
- Create: `src/tui/messages.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Create messages module**

Create `src/tui/messages.rs`:

```rust
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
```

**Step 2: Export from mod.rs**

Add to `src/tui/mod.rs`:

```rust
pub mod messages;
pub use messages::{AgentMessage, AgentSender, AgentReceiver, agent_channel};
```

**Step 3: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/tui/messages.rs src/tui/mod.rs
git commit -m "feat(tui): add agent message types and channel"
```

---

### Task 2: Refactor Event Loop to Async

**Files:**
- Modify: `src/tui/mod.rs`
- Modify: `src/main.rs`

**Step 1: Update run and run_loop to async**

Replace the `run` and `run_loop` functions in `src/tui/mod.rs`:

```rust
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use crate::tui::messages::AgentMessage;

/// Run the TUI event loop with panic safety.
pub async fn run(
    terminal: &mut Tui,
    app: &mut App,
    agent_rx: &mut Receiver<AgentMessage>,
) -> anyhow::Result<()> {
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let result = run_loop(terminal, app, agent_rx).await;

    let _ = panic::take_hook();

    result
}

async fn run_loop(
    terminal: &mut Tui,
    app: &mut App,
    agent_rx: &mut Receiver<AgentMessage>,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(50));

    loop {
        if !app.running {
            break;
        }

        terminal.draw(|frame| crate::tui::ui::draw(frame, app))?;

        tokio::select! {
            _ = interval.tick() => {
                // Poll for terminal events (non-blocking)
                while event::poll(Duration::ZERO)? {
                    match event::read()? {
                        Event::Key(key) if key.kind == KeyEventKind::Press => {
                            app.handle_key(key);
                        }
                        Event::Resize(_, _) => {
                            // Redraw handled next iteration
                        }
                        _ => {}
                    }
                }
            }

            Some(msg) = agent_rx.recv() => {
                app.handle_agent_message(msg);
            }
        }
    }
    Ok(())
}
```

**Step 2: Update main.rs Tui command**

Update the Commands::Tui match arm in `src/main.rs`:

```rust
        Commands::Tui => {
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;
            let spec_dir = config.rustagent.spec_dir.clone();

            use rustagent::tui::{self, agent_channel};

            let mut terminal = tui::setup_terminal()?;
            let (tx, mut rx) = agent_channel();
            let mut app = tui::App::new(&spec_dir, tx);

            let result = tui::run(&mut terminal, &mut app, &mut rx).await;

            tui::restore_terminal(&mut terminal)?;

            result?;
        }
```

**Step 3: Verify**

Run: `cargo check`
Expected: Will fail because `App::new` signature changed and `handle_agent_message` doesn't exist yet. That's expected - we'll fix in next tasks.

**Step 4: Commit (partial - don't commit yet, continue to Task 3)**

---

### Task 3: Update App with Agent Sender and Message Handler

**Files:**
- Modify: `src/tui/app.rs`

**Step 1: Add agent_tx and spec_dir to App**

Update `src/tui/app.rs`:

Add imports:
```rust
use crate::tui::messages::{AgentMessage, AgentSender};
use crate::tui::views::{OutputItem, ToolCall};
```

Update App struct:
```rust
pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
    pub dashboard: DashboardState,
    pub planning: PlanningState,
    pub execution: ExecutionState,
    pub side_panel: SidePanel,
    pub spec_dir: String,
    pub agent_tx: AgentSender,
}
```

Update new():
```rust
    pub fn new(spec_dir: &str, agent_tx: AgentSender) -> Self {
        let mut dashboard = DashboardState::new();
        dashboard.load_specs(spec_dir);

        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard,
            planning: PlanningState::new(),
            execution: ExecutionState::new(),
            side_panel: SidePanel::new(),
            spec_dir: spec_dir.to_string(),
            agent_tx,
        }
    }
```

Update Default impl:
```rust
impl Default for App {
    fn default() -> Self {
        let (tx, _) = crate::tui::messages::agent_channel();
        Self::new("", tx)
    }
}
```

**Step 2: Add handle_agent_message method**

Add to `impl App`:

```rust
    pub fn handle_agent_message(&mut self, msg: AgentMessage) {
        match msg {
            // Planning messages
            AgentMessage::PlanningStarted => {
                self.planning.thinking = true;
            }
            AgentMessage::PlanningResponse(text) => {
                self.planning.thinking = false;
                self.planning.add_message(MessageRole::Assistant, text);
            }
            AgentMessage::PlanningToolCall { name: _, args: _ } => {
                // Tool calls happen in background, keep thinking
            }
            AgentMessage::PlanningToolResult { name: _, output: _ } => {
                // Results flow into next response
            }
            AgentMessage::PlanningComplete { spec_path } => {
                self.planning.thinking = false;
                self.planning.add_message(
                    MessageRole::Assistant,
                    format!("✓ Spec saved to {}", spec_path),
                );
                self.dashboard.load_specs(&self.spec_dir);
            }
            AgentMessage::PlanningError(err) => {
                self.planning.thinking = false;
                self.planning.add_message(
                    MessageRole::Assistant,
                    format!("Error: {}", err),
                );
            }

            // Execution messages
            AgentMessage::ExecutionStarted { spec_path: _ } => {
                self.execution.running = true;
                self.execution.output.clear();
            }
            AgentMessage::TaskStarted { task_id: _, title } => {
                self.execution.add_output(OutputItem::Message {
                    role: "System".to_string(),
                    content: format!("Starting task: {}", title),
                });
            }
            AgentMessage::TaskResponse(text) => {
                self.execution.add_output(OutputItem::Message {
                    role: "Assistant".to_string(),
                    content: text,
                });
            }
            AgentMessage::TaskToolCall { name, args } => {
                self.execution.add_output(OutputItem::ToolCall(ToolCall {
                    name,
                    output: format!("Args: {}", args),
                    collapsed: true,
                }));
            }
            AgentMessage::TaskToolResult { name, output } => {
                self.execution.add_output(OutputItem::ToolCall(ToolCall {
                    name,
                    output,
                    collapsed: false,
                }));
            }
            AgentMessage::TaskComplete { task_id } => {
                self.execution.add_output(OutputItem::Message {
                    role: "System".to_string(),
                    content: format!("✓ Task {} complete", task_id),
                });
            }
            AgentMessage::TaskBlocked { task_id, reason } => {
                self.execution.add_output(OutputItem::Message {
                    role: "System".to_string(),
                    content: format!("✗ Task {} blocked: {}", task_id, reason),
                });
            }
            AgentMessage::ExecutionComplete => {
                self.execution.running = false;
                self.execution.add_output(OutputItem::Message {
                    role: "System".to_string(),
                    content: "Execution complete".to_string(),
                });
                self.dashboard.load_specs(&self.spec_dir);
            }
            AgentMessage::ExecutionError(err) => {
                self.execution.running = false;
                self.execution.add_output(OutputItem::Message {
                    role: "Error".to_string(),
                    content: err,
                });
            }
        }
    }
```

**Step 3: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): add async event loop with agent message handling"
```

---

### Task 4: Add run_with_sender to PlanningAgent

**Files:**
- Modify: `src/planning/mod.rs`

**Step 1: Add imports**

Add to top of `src/planning/mod.rs`:

```rust
use crate::tui::messages::{AgentMessage, AgentSender};
```

**Step 2: Add run_with_sender method**

Add to `impl PlanningAgent`:

```rust
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
                        }).await?;

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
                        }).await?;

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
```

**Step 3: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/planning/mod.rs
git commit -m "feat(planning): add run_with_sender for TUI integration"
```

---

### Task 5: Add run_with_sender to RalphLoop

**Files:**
- Modify: `src/ralph/mod.rs`

**Step 1: Add imports**

Add to top of `src/ralph/mod.rs`:

```rust
use crate::tui::messages::{AgentMessage, AgentSender};
```

**Step 2: Add run_with_sender method**

Add to `impl RalphLoop`:

```rust
    /// Run execution with a message sender for TUI integration
    pub async fn run_with_sender(&self, tx: AgentSender) -> anyhow::Result<()> {
        tx.send(AgentMessage::ExecutionStarted {
            spec_path: self.spec_path.clone(),
        }).await?;

        let mut iteration = 0;

        loop {
            iteration += 1;
            if iteration > self.max_iterations {
                tx.send(AgentMessage::ExecutionError(
                    format!("Reached max iterations ({})", self.max_iterations)
                )).await?;
                break;
            }

            let mut spec = Spec::load(&self.spec_path).context("Failed to load spec")?;

            let task = match spec.find_next_task() {
                Some(t) => t.clone(),
                None => {
                    tx.send(AgentMessage::ExecutionComplete).await?;
                    break;
                }
            };

            tx.send(AgentMessage::TaskStarted {
                task_id: task.id.clone(),
                title: task.title.clone(),
            }).await?;

            // Mark task as in progress
            {
                let task_mut = spec
                    .find_task_mut(&task.id)
                    .context("Task not found in spec")?;
                task_mut.status = TaskStatus::InProgress;
            }
            spec.save(&self.spec_path).context("Failed to save spec")?;

            match self.execute_task_with_sender(&task.id, &tx).await {
                Ok(signal) => {
                    let mut spec = Spec::load(&self.spec_path)?;

                    match signal.as_str() {
                        "TASK_COMPLETE" => {
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found")?;
                            task_mut.status = TaskStatus::Complete;
                            task_mut.completed_at = Some(Utc::now());
                            spec.save(&self.spec_path)?;
                            tx.send(AgentMessage::TaskComplete { task_id: task.id }).await?;
                        }
                        "TASK_BLOCKED" => {
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found")?;
                            task_mut.status = TaskStatus::Blocked;
                            spec.save(&self.spec_path)?;
                            tx.send(AgentMessage::TaskBlocked {
                                task_id: task.id,
                                reason: "Task reported blocked".to_string(),
                            }).await?;
                        }
                        _ => {
                            let task_mut = spec
                                .find_task_mut(&task.id)
                                .context("Task not found")?;
                            task_mut.status = TaskStatus::Pending;
                            spec.save(&self.spec_path)?;
                        }
                    }
                }
                Err(e) => {
                    tx.send(AgentMessage::ExecutionError(e.to_string())).await?;
                    break;
                }
            }
        }

        Ok(())
    }

    async fn execute_task_with_sender(
        &self,
        task_id: &str,
        tx: &AgentSender,
    ) -> Result<String> {
        let context = self.build_context(task_id)?;
        let tool_definitions = self.tools.definitions();

        let mut messages = vec![Message::user(context)];

        let max_turns = 50;
        for _turn in 0..max_turns {
            let response = self
                .client
                .chat(messages.clone(), &tool_definitions)
                .await?;

            match response.content {
                ResponseContent::Text(text) => {
                    tx.send(AgentMessage::TaskResponse(text.clone())).await?;

                    if text.contains("TASK_COMPLETE") {
                        return Ok("TASK_COMPLETE".to_string());
                    }
                    if text.contains("TASK_BLOCKED") {
                        return Ok("TASK_BLOCKED".to_string());
                    }

                    messages.push(Message::assistant(text));
                }
                ResponseContent::ToolCalls(tool_calls) => {
                    for tool_call in &tool_calls {
                        if tool_call.name == "signal_completion" {
                            let tool = self
                                .tools
                                .get(&tool_call.name)
                                .context("signal_completion tool not found")?;
                            let result = tool.execute(tool_call.parameters.clone()).await?;

                            if result.starts_with("SIGNAL:complete:") {
                                return Ok("TASK_COMPLETE".to_string());
                            } else if result.starts_with("SIGNAL:blocked:") {
                                return Ok("TASK_BLOCKED".to_string());
                            }
                        }
                    }

                    let mut results = Vec::new();
                    for tool_call in tool_calls {
                        if tool_call.name == "signal_completion" {
                            continue;
                        }

                        tx.send(AgentMessage::TaskToolCall {
                            name: tool_call.name.clone(),
                            args: tool_call.parameters.to_string(),
                        }).await?;

                        let tool = self.tools.get(&tool_call.name).context("Tool not found")?;

                        match tool.execute(tool_call.parameters).await {
                            Ok(output) => {
                                tx.send(AgentMessage::TaskToolResult {
                                    name: tool_call.name.clone(),
                                    output: output.clone(),
                                }).await?;
                                results.push(format!("Tool: {}\nResult: {}", tool_call.name, output));
                            }
                            Err(e) => {
                                tx.send(AgentMessage::TaskToolResult {
                                    name: tool_call.name.clone(),
                                    output: format!("Error: {}", e),
                                }).await?;
                                results.push(format!("Tool: {}\nError: {}", tool_call.name, e));
                            }
                        }
                    }

                    let results_text = results.join("\n\n");
                    if !results_text.is_empty() {
                        messages.push(Message::user(results_text));
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Reached max turns without completion"))
    }
```

**Step 3: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/ralph/mod.rs
git commit -m "feat(ralph): add run_with_sender for TUI integration"
```

---

### Task 6: Wire Up Planning Submit to Spawn Agent

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/main.rs`

**Step 1: Update App to hold config for agent creation**

Add to App struct in `src/tui/app.rs`:

```rust
use crate::config::Config;

pub struct App {
    // ... existing fields
    pub config: Option<Config>,
}
```

Update new() to accept config:

```rust
    pub fn new(spec_dir: &str, agent_tx: AgentSender, config: Option<Config>) -> Self {
        // ...
        Self {
            // ... existing fields
            config,
        }
    }
```

**Step 2: Spawn planning agent on Enter in planning view**

Update the handle_key Enter case in planning insert mode:

```rust
                KeyCode::Enter => {
                    if let Some(text) = self.planning.submit_input() {
                        self.planning.add_message(MessageRole::User, text.clone());

                        // Spawn planning agent if we have config
                        if let Some(ref config) = self.config {
                            let tx = self.agent_tx.clone();
                            let spec_dir = self.spec_dir.clone();
                            let config = config.clone();

                            tokio::spawn(async move {
                                match crate::planning::PlanningAgent::new(config, spec_dir) {
                                    Ok(mut agent) => {
                                        if let Err(e) = agent.run_with_sender(tx.clone(), text).await {
                                            let _ = tx.send(AgentMessage::PlanningError(e.to_string())).await;
                                        }
                                    }
                                    Err(e) => {
                                        let _ = tx.send(AgentMessage::PlanningError(e.to_string())).await;
                                    }
                                }
                            });
                        }
                    }
                }
```

**Step 3: Update main.rs to pass config**

```rust
        Commands::Tui => {
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;
            let spec_dir = config.rustagent.spec_dir.clone();

            use rustagent::tui::{self, agent_channel};

            let mut terminal = tui::setup_terminal()?;
            let (tx, mut rx) = agent_channel();
            let mut app = tui::App::new(&spec_dir, tx, Some(config));

            let result = tui::run(&mut terminal, &mut app, &mut rx).await;

            tui::restore_terminal(&mut terminal)?;

            result?;
        }
```

**Step 4: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/tui/app.rs src/main.rs
git commit -m "feat(tui): wire planning view to spawn agent on submit"
```

---

### Task 7: Wire Up Dashboard to Spawn Execution

**Files:**
- Modify: `src/tui/app.rs`
- Modify: `src/tui/views/dashboard.rs`

**Step 1: Add method to get selected spec path**

Add to `impl DashboardState` in `src/tui/views/dashboard.rs`:

```rust
    pub fn selected_spec(&self) -> Option<&SpecSummary> {
        let statuses = [SpecStatus::Draft, SpecStatus::Ready, SpecStatus::Running, SpecStatus::Completed];
        let status = statuses.get(self.selected_column)?;

        self.specs
            .iter()
            .filter(|s| s.status == *status)
            .nth(self.selected_row)
    }
```

Also add `spec_path` field to SpecSummary:
```rust
#[derive(Debug, Clone)]
pub struct SpecSummary {
    pub name: String,
    pub path: String,  // Add this
    pub status: SpecStatus,
    pub task_progress: (usize, usize),
}
```

Update `load_specs` to store the path.

**Step 2: Handle Enter key in Dashboard**

Add to handle_key in `src/tui/app.rs`:

```rust
            (KeyCode::Enter, KeyModifiers::NONE) if self.active_tab == ActiveTab::Dashboard => {
                if let Some(spec) = self.dashboard.selected_spec() {
                    let spec_path = spec.path.clone();

                    if let Some(ref config) = self.config {
                        let tx = self.agent_tx.clone();
                        let config = config.clone();

                        // Switch to execution tab
                        self.active_tab = ActiveTab::Execution;

                        tokio::spawn(async move {
                            match crate::ralph::RalphLoop::new(config, spec_path, None) {
                                Ok(ralph) => {
                                    if let Err(e) = ralph.run_with_sender(tx.clone()).await {
                                        let _ = tx.send(AgentMessage::ExecutionError(e.to_string())).await;
                                    }
                                }
                                Err(e) => {
                                    let _ = tx.send(AgentMessage::ExecutionError(e.to_string())).await;
                                }
                            }
                        });
                    }
                }
            }
```

**Step 3: Verify**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): wire dashboard Enter to spawn execution"
```

---

## Phase 2: Polish Features

### Task 8: Add Spinner Widget

**Files:**
- Create: `src/tui/widgets/spinner.rs`
- Modify: `src/tui/widgets/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Create spinner widget**

Create `src/tui/widgets/spinner.rs`:

```rust
const SPINNER_FRAMES: &[char] = &['◐', '◓', '◑', '◒'];

pub struct Spinner {
    frame: usize,
}

impl Spinner {
    pub fn new() -> Self {
        Self { frame: 0 }
    }

    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
    }

    pub fn current(&self) -> char {
        SPINNER_FRAMES[self.frame]
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Export from widgets**

Add to `src/tui/widgets/mod.rs`:

```rust
mod spinner;
pub use spinner::Spinner;
```

**Step 3: Add spinner to App and tick in event loop**

Add to App struct:
```rust
pub spinner: Spinner,
```

Initialize in new().

In `run_loop`, tick the spinner on each interval:
```rust
            _ = interval.tick() => {
                app.spinner.tick();
                // ... existing event polling
            }
```

**Step 4: Update planning view to use spinner**

In `src/tui/views/planning.rs`, update the thinking indicator to use `app.spinner.current()` (requires passing spinner to draw function or storing in PlanningState).

**Step 5: Verify and commit**

```bash
git add src/tui/
git commit -m "feat(tui): add animated spinner widget"
```

---

### Task 9: Add Help Overlay

**Files:**
- Create: `src/tui/widgets/help.rs`
- Modify: `src/tui/widgets/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Create help widget**

Create `src/tui/widgets/help.rs`:

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

pub struct HelpOverlay {
    pub visible: bool,
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self { visible: false }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_help(frame: &mut Frame, area: Rect) {
    // Center the help dialog
    let width = 50.min(area.width.saturating_sub(4));
    let height = 16.min(area.height.saturating_sub(4));
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let help_area = Rect::new(x, y, width, height);

    frame.render_widget(Clear, help_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Help (Esc to close) ");

    let lines = vec![
        Line::from(Span::styled("Global", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  q          Quit"),
        Line::from("  1/2/3      Switch tabs"),
        Line::from("  Tab        Cycle tabs"),
        Line::from("  [ ]        Toggle side panel"),
        Line::from("  ?          Toggle help"),
        Line::from(""),
        Line::from(Span::styled("Dashboard", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  K/A        Kanban/Activity view"),
        Line::from("  ↑↓←→       Navigate"),
        Line::from("  Enter      Run selected spec"),
        Line::from(""),
        Line::from(Span::styled("Planning", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  i          Enter insert mode"),
        Line::from("  Esc        Exit insert mode"),
        Line::from("  Enter      Send message"),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, help_area);
}
```

**Step 2: Export and integrate**

Add to widgets/mod.rs, App struct, handle `?` key, render in ui.rs after side panel.

**Step 3: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): add help overlay with ? toggle"
```

---

### Task 10: Add Kanban Navigation

**Files:**
- Modify: `src/tui/views/dashboard.rs`
- Modify: `src/tui/app.rs`

**Step 1: Add navigation methods to DashboardState**

```rust
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

impl DashboardState {
    pub fn move_selection(&mut self, direction: Direction) {
        match direction {
            Direction::Left => {
                self.selected_column = self.selected_column.saturating_sub(1);
                self.selected_row = 0;
            }
            Direction::Right => {
                self.selected_column = (self.selected_column + 1).min(3);
                self.selected_row = 0;
            }
            Direction::Up => {
                self.selected_row = self.selected_row.saturating_sub(1);
            }
            Direction::Down => {
                let count = self.specs_in_current_column().len();
                if count > 0 {
                    self.selected_row = (self.selected_row + 1).min(count - 1);
                }
            }
        }
    }

    fn specs_in_current_column(&self) -> Vec<&SpecSummary> {
        let statuses = [SpecStatus::Draft, SpecStatus::Ready, SpecStatus::Running, SpecStatus::Completed];
        if let Some(status) = statuses.get(self.selected_column) {
            self.specs.iter().filter(|s| s.status == *status).collect()
        } else {
            Vec::new()
        }
    }
}
```

**Step 2: Handle arrow keys in app.rs**

Add to handle_key for Dashboard:

```rust
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.move_selection(Direction::Up);
            }
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.move_selection(Direction::Down);
            }
            (KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('h'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.move_selection(Direction::Left);
            }
            (KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.move_selection(Direction::Right);
            }
```

**Step 3: Verify and commit**

```bash
git add src/tui/
git commit -m "feat(tui): add kanban navigation with arrow keys"
```

---

## Summary

**Phase 1: Agent Integration (Tasks 1-7)**
1. Message types and channel
2. Async event loop with tokio::select
3. App message handler
4. PlanningAgent run_with_sender
5. RalphLoop run_with_sender
6. Planning submit → spawn agent
7. Dashboard Enter → spawn execution

**Phase 2: Polish (Tasks 8-10)**
8. Spinner widget
9. Help overlay
10. Kanban navigation
