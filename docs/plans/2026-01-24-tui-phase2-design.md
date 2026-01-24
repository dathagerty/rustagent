# TUI Phase 2 Design Document

**Date:** 2026-01-24
**Status:** Draft

## Overview

Extend the TUI with async agent integration and polish features. The TUI will spawn PlanningAgent and RalphLoop in background tasks, receiving structured updates via tokio channels.

## Phase 1: Agent Integration

### Message Types

```rust
// src/tui/messages.rs

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

pub type AgentSender = tokio::sync::mpsc::Sender<AgentMessage>;
pub type AgentReceiver = tokio::sync::mpsc::Receiver<AgentMessage>;

pub fn agent_channel() -> (AgentSender, AgentReceiver) {
    tokio::sync::mpsc::channel(100)
}
```

### Event Loop Refactoring

The run loop becomes async and uses `tokio::select!` to multiplex:
- Terminal events (polled with interval)
- Agent messages (received from channel)

```rust
async fn run_loop(
    terminal: &mut Tui,
    app: &mut App,
    agent_rx: &mut Receiver<AgentMessage>,
) -> anyhow::Result<()> {
    let mut interval = tokio::time::interval(Duration::from_millis(100));

    loop {
        if !app.running {
            break;
        }

        terminal.draw(|frame| crate::tui::ui::draw(frame, app))?;

        tokio::select! {
            _ = interval.tick() => {
                while event::poll(Duration::ZERO)? {
                    if let Event::Key(key) = event::read()? {
                        if key.kind == KeyEventKind::Press {
                            app.handle_key(key);
                        }
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

### Message Handling in App

App routes messages to appropriate view state:

- `PlanningResponse` → `planning.add_message()`
- `PlanningStarted` → `planning.thinking = true`
- `TaskResponse` → `execution.add_output(OutputItem::Message)`
- `TaskToolCall/Result` → `execution.add_output(OutputItem::ToolCall)`
- `PlanningComplete` → reload dashboard specs
- `ExecutionComplete` → `execution.running = false`

App stores `spec_dir: String` and `agent_tx: AgentSender` for spawning.

### Agent Refactoring

Both agents get new methods for TUI mode:

**PlanningAgent**
```rust
pub async fn run_with_sender(
    &mut self,
    tx: AgentSender,
    initial_message: String,
) -> anyhow::Result<()>
```

**RalphLoop**
```rust
pub async fn run_with_sender(
    &self,
    tx: AgentSender,
) -> anyhow::Result<()>
```

These replace `println!` calls with `tx.send()`. Existing `run()` methods remain for CLI usage.

### Spawning from TUI

**Planning:** When user submits message in Planning view:
```rust
tokio::spawn(async move {
    agent.run_with_sender(tx, user_input).await
});
```

**Execution:** When user selects spec in Dashboard and presses Enter:
```rust
tokio::spawn(async move {
    ralph.run_with_sender(tx).await
});
```

---

## Phase 2: Polish Features

### Spinner Animation

```rust
// src/tui/widgets/spinner.rs

const SPINNER_FRAMES: &[char] = &['◐', '◓', '◑', '◒'];

pub struct Spinner {
    frame: usize,
}

impl Spinner {
    pub fn tick(&mut self) {
        self.frame = (self.frame + 1) % SPINNER_FRAMES.len();
    }

    pub fn current(&self) -> char {
        SPINNER_FRAMES[self.frame]
    }
}
```

- App holds `spinner: Spinner`
- Tick on each interval in event loop
- Views use `app.spinner.current()` when rendering thinking/running state

### Help Overlay

```rust
// src/tui/widgets/help.rs

pub struct HelpOverlay {
    pub visible: bool,
}
```

Keybindings displayed:
- **Global:** q quit, 1/2/3 tabs, Tab cycle, [ ] panel, ? help
- **Dashboard:** K kanban, A activity, ↑↓←→ navigate, Enter run
- **Planning:** i insert, Esc exit, Enter send, Ctrl+S save
- **Execution:** Ctrl+X stop, j/k scroll, r resume

Toggle with `?`, close with `Esc`. Rendered as centered modal on top of all content.

### Kanban Navigation

```rust
impl DashboardState {
    pub fn move_selection(&mut self, direction: Direction) {
        match direction {
            Direction::Left => self.selected_column = self.selected_column.saturating_sub(1),
            Direction::Right => self.selected_column = (self.selected_column + 1).min(3),
            Direction::Up => self.selected_row = self.selected_row.saturating_sub(1),
            Direction::Down => {
                // Clamp to specs in current column
                let count = self.specs_in_column(self.selected_column).len();
                self.selected_row = (self.selected_row + 1).min(count.saturating_sub(1));
            }
        }
    }
}
```

- Arrow keys or h/j/k/l for navigation
- Enter on selected spec loads it into Execution view and switches tab

---

## Implementation Order

### Phase 1: Agent Integration
1. Add `src/tui/messages.rs`
2. Refactor `run()` and `run_loop()` to async with `tokio::select!`
3. Add `handle_agent_message()` to App
4. Store `spec_dir` and `agent_tx` in App
5. Add `run_with_sender()` to PlanningAgent
6. Add `run_with_sender()` to RalphLoop
7. Wire up Planning view submit → spawn agent
8. Wire up Dashboard Enter → spawn execution

### Phase 2: Polish
9. Add Spinner widget and tick in event loop
10. Add HelpOverlay widget with `?` toggle
11. Add Kanban navigation (arrows, Enter action)

---

## Out of Scope

- Session save/restore
- Multiple concurrent agent runs
- Streaming token-by-token output
