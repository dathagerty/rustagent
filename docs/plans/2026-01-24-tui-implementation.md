# TUI Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an interactive TUI for rustagent with Dashboard, Planning, and Execution views.

**Architecture:** Ratatui-based TUI with tab navigation, slide-in panels, and async message streaming from existing PlanningAgent and RalphLoop via tokio channels.

**Tech Stack:** ratatui, crossterm, tui-textarea, tokio (existing)

---

## Design Decisions

### Terminal Lifecycle Safety
- Always restore terminal on exit, error, or panic using `scopeguard` or manual drop guard
- Handle `Event::Resize` to redraw correctly
- Use `KeyEventKind::Press` to filter out key repeats/releases

### Input Model
- Use full `KeyEvent` (not just `KeyCode`) to preserve modifiers for tui-textarea
- Planning view needs Ctrl+S, Ctrl+X, etc.

### Output Management
- Cap execution output to last 1000 items to prevent unbounded growth
- Clamp scroll offsets to `u16::MAX` and content bounds

### Async Integration (future)
- Dedicated OS thread for terminal events, forward via channel
- Or use crossterm async event stream with tokio
- Bounded channels with backpressure for agent messages

---

## Task 1: Add TUI Dependencies

**Files:**
- Modify: `Cargo.toml`

**Step 1: Add ratatui and related dependencies**

Add to `[dependencies]` section in `Cargo.toml`:

```toml
ratatui = "0.29"
crossterm = "0.28"
tui-textarea = { version = "0.7", default-features = false, features = ["crossterm"] }
```

Note: `tui-textarea` requires explicit crossterm feature for ratatui 0.29 compatibility.

**Step 2: Verify dependencies compile**

Run: `cargo check`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "deps: add ratatui, crossterm, tui-textarea for TUI"
```

---

## Task 2: Create TUI Module Skeleton

**Files:**
- Create: `src/tui/mod.rs`
- Create: `src/tui/app.rs`
- Modify: `src/lib.rs`

**Step 1: Create basic app state**

Create `src/tui/app.rs`:

```rust
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Planning,
    Execution,
}

pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
        }
    }

    /// Handle key events. Uses full KeyEvent to preserve modifiers.
    pub fn handle_key(&mut self, key: KeyEvent) {
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => self.running = false,
            (KeyCode::Char('q'), KeyModifiers::NONE) => self.running = false,
            (KeyCode::Char('1'), KeyModifiers::NONE) => self.active_tab = ActiveTab::Dashboard,
            (KeyCode::Char('2'), KeyModifiers::NONE) => self.active_tab = ActiveTab::Planning,
            (KeyCode::Char('3'), KeyModifiers::NONE) => self.active_tab = ActiveTab::Execution,
            (KeyCode::Tab, _) => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Dashboard => ActiveTab::Planning,
                    ActiveTab::Planning => ActiveTab::Execution,
                    ActiveTab::Execution => ActiveTab::Dashboard,
                };
            }
            _ => {}
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
```

**Step 2: Create TUI module entry**

Create `src/tui/mod.rs`:

```rust
mod app;

pub use app::{App, ActiveTab};

use std::io;
use std::panic;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    event::{self, Event, KeyEventKind},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};

pub type Tui = Terminal<CrosstermBackend<io::Stdout>>;

pub fn setup_terminal() -> io::Result<Tui> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend)
}

pub fn restore_terminal(terminal: &mut Tui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

/// Run the TUI event loop with panic safety.
/// Terminal is always restored even on panic or error.
pub fn run(terminal: &mut Tui, app: &mut App) -> anyhow::Result<()> {
    // Set panic hook to restore terminal
    let original_hook = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen);
        original_hook(info);
    }));

    let result = run_loop(terminal, app);

    // Restore original panic hook
    let _ = panic::take_hook();

    result
}

fn run_loop(terminal: &mut Tui, app: &mut App) -> anyhow::Result<()> {
    while app.running {
        terminal.draw(|frame| crate::tui::ui::draw(frame, app))?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) if key.kind == KeyEventKind::Press => {
                    app.handle_key(key);
                }
                Event::Resize(_, _) => {
                    // Terminal will redraw on next iteration
                }
                _ => {}
            }
        }
    }
    Ok(())
}
```

**Step 3: Export TUI module from lib.rs**

Add to `src/lib.rs`:

```rust
pub mod tui;
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/tui/ src/lib.rs
git commit -m "feat(tui): add basic app state and terminal setup"
```

---

## Task 3: Implement Tab Bar Widget

**Files:**
- Create: `src/tui/widgets/mod.rs`
- Create: `src/tui/widgets/tabs.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Create tabs widget**

Create `src/tui/widgets/tabs.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Tabs as RataTabs, Widget},
};

use crate::tui::ActiveTab;

pub struct TabBar {
    active: ActiveTab,
}

impl TabBar {
    pub fn new(active: ActiveTab) -> Self {
        Self { active }
    }
}

impl Widget for TabBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        // Use Line::from for ratatui 0.29 compatibility
        let titles = vec![
            Line::from("[1] Dashboard"),
            Line::from("[2] Planning"),
            Line::from("[3] Execution"),
        ];
        let selected = match self.active {
            ActiveTab::Dashboard => 0,
            ActiveTab::Planning => 1,
            ActiveTab::Execution => 2,
        };

        let tabs = RataTabs::new(titles)
            .select(selected)
            .style(Style::default().fg(Color::White))
            .highlight_style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )
            .divider(Span::raw(" │ "));

        tabs.render(area, buf);
    }
}
```

**Step 2: Create widgets module**

Create `src/tui/widgets/mod.rs`:

```rust
mod tabs;

pub use tabs::TabBar;
```

**Step 3: Update TUI mod.rs to include widgets**

Add to `src/tui/mod.rs`:

```rust
pub mod widgets;
```

**Step 4: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 5: Commit**

```bash
git add src/tui/widgets/
git commit -m "feat(tui): add tab bar widget"
```

---

## Task 4: Implement Basic UI Rendering

**Files:**
- Create: `src/tui/ui.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Create UI rendering function**

Create `src/tui/ui.rs`:

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{App, ActiveTab};
use crate::tui::widgets::TabBar;

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),  // Tab bar
            Constraint::Min(0),     // Main content
            Constraint::Length(1),  // Status bar
        ])
        .split(frame.area());

    // Tab bar
    frame.render_widget(TabBar::new(app.active_tab), chunks[0]);

    // Main content area
    let content_block = Block::default()
        .borders(Borders::ALL)
        .title(match app.active_tab {
            ActiveTab::Dashboard => " Dashboard ",
            ActiveTab::Planning => " Planning ",
            ActiveTab::Execution => " Execution ",
        });

    let placeholder = Paragraph::new(match app.active_tab {
        ActiveTab::Dashboard => "Dashboard view - coming soon",
        ActiveTab::Planning => "Planning view - coming soon",
        ActiveTab::Execution => "Execution view - coming soon",
    })
    .block(content_block);

    frame.render_widget(placeholder, chunks[1]);

    // Status bar
    let status = Line::from(vec![
        Span::raw(" q quit │ 1/2/3 switch tabs │ Tab cycle │ ? help "),
    ]);
    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status_bar, chunks[2]);
}
```

**Step 2: Export ui module**

Add to `src/tui/mod.rs`:

```rust
mod ui;

pub use ui::draw;
```

**Step 3: Verify it compiles**

Run: `cargo check`
Expected: Compiles without errors

**Step 4: Commit**

```bash
git add src/tui/ui.rs src/tui/mod.rs
git commit -m "feat(tui): add basic UI layout with tab bar and status bar"
```

---

## Task 5: Add TUI Command to CLI

**Files:**
- Modify: `src/main.rs`

**Step 1: Make TUI the default command**

Update the `Cli` struct in `src/main.rs` to make the subcommand optional:

```rust
#[derive(Parser)]
#[command(name = "rustagent")]
#[command(about = "A Rust-based AI agent for task execution", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}
```

Add to the `Commands` enum:

```rust
    /// Launch interactive TUI
    Tui,
```

**Step 2: Update match statement to handle default**

Replace the match statement in `main()`:

```rust
    // Default to TUI if no command specified
    let command = cli.command.unwrap_or(Commands::Tui);

    match command {
        Commands::Init { spec_dir } => {
            // ... existing init code unchanged
        }
        Commands::Plan { spec_dir } => {
            // ... existing plan code unchanged
        }
        Commands::Run {
            spec_file,
            max_iterations,
        } => {
            // ... existing run code unchanged
        }
        Commands::Tui => {
            use rustagent::tui;

            let mut terminal = tui::setup_terminal()?;
            let mut app = tui::App::new();

            // Run with panic-safe terminal restoration
            let result = tui::run(&mut terminal, &mut app);

            // Always restore terminal
            tui::restore_terminal(&mut terminal)?;

            // Propagate any error from the run loop
            result?;
        }
    }
```

**Step 3: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run`
Expected: TUI launches (default command), tabs switch with 1/2/3, q quits

Run: `cargo run -- tui`
Expected: Same behavior with explicit tui subcommand

**Step 4: Commit**

```bash
git add src/main.rs
git commit -m "feat(cli): add tui subcommand to launch interactive TUI"
```

---

## Task 6: Implement Dashboard Kanban View

**Files:**
- Create: `src/tui/views/mod.rs`
- Create: `src/tui/views/dashboard.rs`
- Modify: `src/tui/ui.rs`
- Modify: `src/tui/mod.rs`

**Step 1: Create dashboard state and view**

Create `src/tui/views/dashboard.rs`:

```rust
use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Widget},
};

use crate::spec::{Spec, TaskStatus};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardMode {
    Kanban,
    Activity,
}

pub struct DashboardState {
    pub mode: DashboardMode,
    pub specs: Vec<SpecSummary>,
    pub selected_column: usize,
    pub selected_row: usize,
}

#[derive(Debug, Clone)]
pub struct SpecSummary {
    pub name: String,
    pub status: SpecStatus,
    pub task_progress: (usize, usize), // (completed, total)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecStatus {
    Draft,
    Ready,
    Running,
    Completed,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            mode: DashboardMode::Kanban,
            specs: Vec::new(),
            selected_column: 0,
            selected_row: 0,
        }
    }

    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            DashboardMode::Kanban => DashboardMode::Activity,
            DashboardMode::Activity => DashboardMode::Kanban,
        };
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_dashboard(frame: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Mode toggle
            Constraint::Min(0),    // Content
        ])
        .split(area);

    // Mode toggle bar
    let mode_text = match state.mode {
        DashboardMode::Kanban => " View: [K]anban │ Activity ",
        DashboardMode::Activity => " View: Kanban │ [A]ctivity ",
    };
    let mode_bar = Paragraph::new(mode_text)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(mode_bar, chunks[0]);

    // Content based on mode
    match state.mode {
        DashboardMode::Kanban => draw_kanban(frame, chunks[1], state),
        DashboardMode::Activity => draw_activity(frame, chunks[1], state),
    }
}

fn draw_kanban(frame: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let column_titles = ["Draft", "Ready", "Running", "Completed"];
    let statuses = [SpecStatus::Draft, SpecStatus::Ready, SpecStatus::Running, SpecStatus::Completed];

    for (i, (col_area, (title, status))) in columns.iter()
        .zip(column_titles.iter().zip(statuses.iter()))
        .enumerate()
    {
        let is_selected = i == state.selected_column;
        let style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(*title)
            .border_style(style);

        let specs_in_column: Vec<&SpecSummary> = state.specs
            .iter()
            .filter(|s| s.status == *status)
            .collect();

        let items: Vec<ListItem> = specs_in_column
            .iter()
            .enumerate()
            .map(|(j, spec)| {
                let content = format!("{} ({}/{})", spec.name, spec.task_progress.0, spec.task_progress.1);
                let item_style = if is_selected && j == state.selected_row {
                    Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(content).style(item_style)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, *col_area);
    }
}

fn draw_activity(frame: &mut ratatui::Frame, area: Rect, _state: &DashboardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Activity Feed ");

    // Header row
    let header = Line::from(vec![
        Span::styled("Time      ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Event             ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Spec         ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Details", Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let content = Paragraph::new(vec![
        header,
        Line::from("─".repeat(area.width.saturating_sub(2) as usize)),
        Line::from("  No activity yet"),
    ])
    .block(block);

    frame.render_widget(content, area);
}
```

**Step 2: Create views module**

Create `src/tui/views/mod.rs`:

```rust
mod dashboard;

pub use dashboard::{DashboardState, DashboardMode, SpecSummary, SpecStatus, draw_dashboard};
```

**Step 3: Update TUI mod.rs**

Add to `src/tui/mod.rs`:

```rust
pub mod views;
```

**Step 4: Update App state to include dashboard state**

Modify `src/tui/app.rs` to add:

```rust
use crate::tui::views::DashboardState;
```

And update the `App` struct:

```rust
pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
    pub dashboard: DashboardState,
}

impl App {
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard: DashboardState::new(),
        }
    }

    pub fn handle_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('1') => self.active_tab = ActiveTab::Dashboard,
            KeyCode::Char('2') => self.active_tab = ActiveTab::Planning,
            KeyCode::Char('3') => self.active_tab = ActiveTab::Execution,
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Dashboard => ActiveTab::Planning,
                    ActiveTab::Planning => ActiveTab::Execution,
                    ActiveTab::Execution => ActiveTab::Dashboard,
                };
            }
            // Dashboard-specific keys
            KeyCode::Char('k') | KeyCode::Char('K') if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = crate::tui::views::DashboardMode::Kanban;
            }
            KeyCode::Char('a') | KeyCode::Char('A') if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = crate::tui::views::DashboardMode::Activity;
            }
            _ => {}
        }
    }
}
```

**Step 5: Update UI to use dashboard view**

Modify `src/tui/ui.rs` to use the dashboard view:

```rust
use crate::tui::views::draw_dashboard;
```

And update the `draw` function to replace the Dashboard placeholder:

```rust
    // Main content area
    match app.active_tab {
        ActiveTab::Dashboard => {
            draw_dashboard(frame, chunks[1], &app.dashboard);
        }
        ActiveTab::Planning => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Planning ");
            let placeholder = Paragraph::new("Planning view - coming soon").block(block);
            frame.render_widget(placeholder, chunks[1]);
        }
        ActiveTab::Execution => {
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" Execution ");
            let placeholder = Paragraph::new("Execution view - coming soon").block(block);
            frame.render_widget(placeholder, chunks[1]);
        }
    }
```

**Step 6: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run -- tui`
Expected: Dashboard shows Kanban with 4 columns, K/A switches modes

**Step 7: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): implement dashboard with kanban and activity views"
```

---

## Task 7: Implement Planning View with Chat Interface

**Files:**
- Create: `src/tui/views/planning.rs`
- Modify: `src/tui/views/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Create planning view state and rendering**

Create `src/tui/views/planning.rs`:

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tui_textarea::TextArea;

#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: MessageRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageRole {
    User,
    Assistant,
}

pub struct PlanningState {
    pub messages: Vec<ChatMessage>,
    pub input: TextArea<'static>,
    pub insert_mode: bool,
    pub scroll_offset: usize,
    pub thinking: bool,
}

impl PlanningState {
    pub fn new() -> Self {
        let mut input = TextArea::default();
        input.set_cursor_line_style(Style::default());
        input.set_placeholder_text("Type your message...");

        Self {
            messages: vec![ChatMessage {
                role: MessageRole::Assistant,
                content: "What would you like to build?".to_string(),
            }],
            input,
            insert_mode: false,
            scroll_offset: 0,
            thinking: false,
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage { role, content });
    }

    pub fn submit_input(&mut self) -> Option<String> {
        let text: String = self.input.lines().join("\n");
        if text.trim().is_empty() {
            return None;
        }
        self.input.select_all();
        self.input.cut();
        Some(text)
    }
}

impl Default for PlanningState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_planning(frame: &mut Frame, area: Rect, state: &mut PlanningState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Chat history
            Constraint::Length(3), // Input area
            Constraint::Length(1), // Hints
        ])
        .split(area);

    // Chat history
    draw_chat_history(frame, chunks[0], state);

    // Input area
    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.insert_mode {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        })
        .title(if state.insert_mode { " Input (INSERT) " } else { " Input " });

    state.input.set_block(input_block);
    frame.render_widget(&state.input, chunks[1]);

    // Hints
    let hints = if state.insert_mode {
        " Esc exit insert │ Enter send "
    } else {
        " i insert │ ↑↓ scroll │ Ctrl+S save │ ] spec panel "
    };
    let hints_bar = Paragraph::new(hints)
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hints_bar, chunks[2]);
}

fn draw_chat_history(frame: &mut Frame, area: Rect, state: &PlanningState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let (label, style) = match msg.role {
            MessageRole::User => ("You", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            MessageRole::Assistant => ("Assistant", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        };

        lines.push(Line::from(Span::styled(label, style)));

        for line in msg.content.lines() {
            lines.push(Line::from(format!("  {}", line)));
        }
        lines.push(Line::from(""));
    }

    if state.thinking {
        lines.push(Line::from(vec![
            Span::styled("Assistant", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" "),
            Span::styled("◐", Style::default().fg(Color::Yellow)),
        ]));
    }

    // Clamp scroll offset to u16::MAX to prevent overflow
    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
}
```

**Step 2: Export planning view**

Add to `src/tui/views/mod.rs`:

```rust
mod planning;

pub use planning::{PlanningState, ChatMessage, MessageRole, draw_planning};
```

**Step 3: Update App with planning state**

Modify `src/tui/app.rs` to include planning state and handle keys:

Add import:
```rust
use crate::tui::views::{DashboardState, DashboardMode, PlanningState};
use crossterm::event::KeyCode;
```

Update App struct:
```rust
pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
    pub dashboard: DashboardState,
    pub planning: PlanningState,
}
```

Update new():
```rust
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard: DashboardState::new(),
            planning: PlanningState::new(),
        }
    }
```

Update handle_key() to handle planning input:
```rust
    pub fn handle_key(&mut self, key: KeyCode) {
        // Planning insert mode handling
        if self.active_tab == ActiveTab::Planning && self.planning.insert_mode {
            match key {
                KeyCode::Esc => {
                    self.planning.insert_mode = false;
                }
                KeyCode::Enter => {
                    if let Some(text) = self.planning.submit_input() {
                        self.planning.add_message(
                            crate::tui::views::MessageRole::User,
                            text,
                        );
                        // TODO: Send to planning agent
                    }
                }
                _ => {
                    self.planning.input.input(crossterm::event::KeyEvent::new(key, crossterm::event::KeyModifiers::NONE));
                }
            }
            return;
        }

        match key {
            KeyCode::Char('q') => self.running = false,
            KeyCode::Char('1') => self.active_tab = ActiveTab::Dashboard,
            KeyCode::Char('2') => self.active_tab = ActiveTab::Planning,
            KeyCode::Char('3') => self.active_tab = ActiveTab::Execution,
            KeyCode::Tab => {
                self.active_tab = match self.active_tab {
                    ActiveTab::Dashboard => ActiveTab::Planning,
                    ActiveTab::Planning => ActiveTab::Execution,
                    ActiveTab::Execution => ActiveTab::Dashboard,
                };
            }
            // Dashboard keys
            KeyCode::Char('k') | KeyCode::Char('K') if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Kanban;
            }
            KeyCode::Char('a') | KeyCode::Char('A') if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Activity;
            }
            // Planning keys
            KeyCode::Char('i') if self.active_tab == ActiveTab::Planning => {
                self.planning.insert_mode = true;
            }
            _ => {}
        }
    }
```

**Step 4: Update UI to render planning view**

Modify `src/tui/ui.rs`:

Add import:
```rust
use crate::tui::views::{draw_dashboard, draw_planning};
```

Update the Planning arm in draw():
```rust
        ActiveTab::Planning => {
            draw_planning(frame, chunks[1], &mut app.planning);
        }
```

Note: This requires changing the `app` parameter to `&mut App`.

**Step 5: Update main.rs to pass mutable app**

Update the draw call in main.rs:
```rust
terminal.draw(|frame| tui::draw(frame, &mut app))?;
```

**Step 6: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run -- tui`
Expected: Planning tab shows chat, i enters insert mode, Esc exits, Enter submits

**Step 7: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): implement planning view with chat interface"
```

---

## Task 8: Implement Execution View with Split Layout

**Files:**
- Create: `src/tui/views/execution.rs`
- Modify: `src/tui/views/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Create execution view**

Create `src/tui/views/execution.rs`:

```rust
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::spec::{Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub output: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone)]
pub enum OutputItem {
    Message { role: String, content: String },
    ToolCall(ToolCall),
}

const MAX_OUTPUT_ITEMS: usize = 1000;

pub struct ExecutionState {
    pub running: bool,
    pub current_task: Option<Task>,
    pub tasks: Vec<Task>,
    pub output: Vec<OutputItem>,
    pub scroll_offset: usize,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            running: false,
            current_task: None,
            tasks: Vec::new(),
            output: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn task_progress(&self) -> (usize, usize) {
        let completed = self.tasks.iter()
            .filter(|t| t.status == TaskStatus::Complete)
            .count();
        (completed, self.tasks.len())
    }

    /// Add output item, capping total to MAX_OUTPUT_ITEMS
    pub fn add_output(&mut self, item: OutputItem) {
        self.output.push(item);
        if self.output.len() > MAX_OUTPUT_ITEMS {
            self.output.remove(0);
            // Adjust scroll offset if we removed content above viewport
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
    }

    /// Clamp scroll offset to valid range
    pub fn clamp_scroll(&mut self, content_height: usize, viewport_height: usize) {
        let max_scroll = content_height.saturating_sub(viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_execution(frame: &mut Frame, area: Rect, state: &ExecutionState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Task pane
            Constraint::Percentage(65), // Output pane
        ])
        .split(area);

    draw_task_pane(frame, chunks[0], state);
    draw_output_pane(frame, chunks[1], state);
}

fn draw_task_pane(frame: &mut Frame, area: Rect, state: &ExecutionState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8), // Current task
            Constraint::Min(0),    // Task list
        ])
        .split(area);

    // Current task
    let (completed, total) = state.task_progress();
    let title = format!(" Current Task {}/{} ", completed, total);
    let current_block = Block::default()
        .borders(Borders::ALL)
        .title(title);

    let current_content = if let Some(task) = &state.current_task {
        let mut lines = vec![
            Line::from(Span::styled(
                format!("▶ {}", task.title),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled("Acceptance Criteria:", Style::default().fg(Color::Cyan))),
        ];
        for criterion in &task.acceptance_criteria {
            lines.push(Line::from(format!(" ☐ {}", criterion)));
        }
        lines
    } else {
        vec![Line::from("No task running")]
    };

    let current = Paragraph::new(current_content)
        .block(current_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(current, chunks[0]);

    // Task list
    let tasks_block = Block::default()
        .borders(Borders::ALL)
        .title(" Tasks ");

    let items: Vec<ListItem> = state.tasks.iter().map(|task| {
        let (icon, style) = match task.status {
            TaskStatus::Complete => ("✓", Style::default().fg(Color::Green)),
            TaskStatus::InProgress => ("▶", Style::default().fg(Color::Yellow)),
            TaskStatus::Pending => ("○", Style::default().fg(Color::DarkGray)),
            TaskStatus::Blocked => ("✗", Style::default().fg(Color::Red)),
        };
        ListItem::new(format!(" {} {}", icon, task.title)).style(style)
    }).collect();

    let list = List::new(items).block(tasks_block);
    frame.render_widget(list, chunks[1]);
}

fn draw_output_pane(frame: &mut Frame, area: Rect, state: &ExecutionState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Output ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for item in &state.output {
        match item {
            OutputItem::Message { role, content } => {
                let style = if role == "Assistant" {
                    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
                };
                lines.push(Line::from(Span::styled(role.clone(), style)));
                for line in content.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
                lines.push(Line::from(""));
            }
            OutputItem::ToolCall(tc) => {
                lines.push(Line::from(vec![
                    Span::raw("┌─ "),
                    Span::styled(&tc.name, Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                    Span::raw(" ─"),
                ]));
                if !tc.collapsed {
                    for line in tc.output.lines().take(5) {
                        lines.push(Line::from(format!("│ {}", line)));
                    }
                }
                lines.push(Line::from("└────────────────────"));
                lines.push(Line::from(""));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("  Waiting for execution..."));
    }

    // Clamp scroll offset to u16::MAX to prevent overflow
    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
}
```

**Step 2: Export execution view**

Add to `src/tui/views/mod.rs`:

```rust
mod execution;

pub use execution::{ExecutionState, OutputItem, ToolCall, draw_execution};
```

**Step 3: Update App with execution state**

Add to imports in `src/tui/app.rs`:
```rust
use crate::tui::views::{DashboardState, DashboardMode, PlanningState, ExecutionState};
```

Update App struct:
```rust
pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
    pub dashboard: DashboardState,
    pub planning: PlanningState,
    pub execution: ExecutionState,
}
```

Update new():
```rust
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard: DashboardState::new(),
            planning: PlanningState::new(),
            execution: ExecutionState::new(),
        }
    }
```

**Step 4: Update UI to render execution view**

Add import in `src/tui/ui.rs`:
```rust
use crate::tui::views::{draw_dashboard, draw_planning, draw_execution};
```

Update the Execution arm:
```rust
        ActiveTab::Execution => {
            draw_execution(frame, chunks[1], &app.execution);
        }
```

**Step 5: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run -- tui`
Expected: Execution tab shows split layout with task pane and output pane

**Step 6: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): implement execution view with split layout"
```

---

## Task 9: Add Slide-in Side Panel

**Files:**
- Create: `src/tui/widgets/panel.rs`
- Modify: `src/tui/widgets/mod.rs`
- Modify: `src/tui/app.rs`
- Modify: `src/tui/ui.rs`

**Step 1: Create panel widget**

Create `src/tui/widgets/panel.rs`:

```rust
use ratatui::{
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

pub struct SidePanel {
    pub visible: bool,
    pub title: String,
    pub content: String,
    pub width_percent: u16,
}

impl SidePanel {
    pub fn new() -> Self {
        Self {
            visible: false,
            title: String::new(),
            content: String::new(),
            width_percent: 40,
        }
    }

    pub fn toggle(&mut self) {
        self.visible = !self.visible;
    }

    pub fn set_content(&mut self, title: &str, content: String) {
        self.title = title.to_string();
        self.content = content;
    }
}

impl Default for SidePanel {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_side_panel(frame: &mut Frame, area: Rect, panel: &SidePanel) {
    if !panel.visible {
        return;
    }

    let panel_width = (area.width as u32 * panel.width_percent as u32 / 100) as u16;
    let panel_area = Rect {
        x: area.x + area.width - panel_width,
        y: area.y,
        width: panel_width,
        height: area.height,
    };

    // Clear the area first
    frame.render_widget(Clear, panel_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(format!(" {} ", panel.title));

    let content = Paragraph::new(panel.content.clone())
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(content, panel_area);
}
```

**Step 2: Export panel widget**

Add to `src/tui/widgets/mod.rs`:

```rust
mod panel;

pub use panel::{SidePanel, draw_side_panel};
```

**Step 3: Add panel to App state**

Add to `src/tui/app.rs`:

```rust
use crate::tui::widgets::SidePanel;
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
}
```

Update new():
```rust
    pub fn new() -> Self {
        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard: DashboardState::new(),
            planning: PlanningState::new(),
            execution: ExecutionState::new(),
            side_panel: SidePanel::new(),
        }
    }
```

Add panel toggle to handle_key():
```rust
            KeyCode::Char('[') | KeyCode::Char(']') => {
                self.side_panel.toggle();
            }
            KeyCode::Esc => {
                if self.side_panel.visible {
                    self.side_panel.visible = false;
                }
            }
```

**Step 4: Render panel in UI**

Add import in `src/tui/ui.rs`:
```rust
use crate::tui::widgets::{TabBar, draw_side_panel};
```

Add panel rendering at end of draw():
```rust
    // Side panel overlay (rendered last)
    draw_side_panel(frame, chunks[1], &app.side_panel);
```

**Step 5: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run -- tui`
Expected: [ or ] toggles side panel, Esc closes it

**Step 6: Commit**

```bash
git add src/tui/
git commit -m "feat(tui): add slide-in side panel widget"
```

---

## Task 10: Load Specs from Disk in Dashboard

**Files:**
- Modify: `src/tui/views/dashboard.rs`
- Modify: `src/tui/app.rs`

**Step 1: Add spec loading to dashboard**

Add to `src/tui/views/dashboard.rs`:

```rust
use std::path::Path;
use std::fs;
use crate::spec::Spec;

impl DashboardState {
    pub fn load_specs(&mut self, spec_dir: &str) {
        self.specs.clear();

        let spec_path = Path::new(spec_dir);
        if !spec_path.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(spec_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Look for spec.json files
                let spec_file = if path.is_dir() {
                    path.join("spec.json")
                } else if path.extension().map_or(false, |e| e == "json") {
                    path
                } else {
                    continue;
                };

                if let Ok(spec) = Spec::load(&spec_file) {
                    let completed = spec.tasks.iter()
                        .filter(|t| t.status == crate::spec::TaskStatus::Complete)
                        .count();
                    let total = spec.tasks.len();

                    let status = if completed == total && total > 0 {
                        SpecStatus::Completed
                    } else if spec.tasks.iter().any(|t| t.status == crate::spec::TaskStatus::InProgress) {
                        SpecStatus::Running
                    } else if total > 0 {
                        SpecStatus::Ready
                    } else {
                        SpecStatus::Draft
                    };

                    self.specs.push(SpecSummary {
                        name: spec.name,
                        status,
                        task_progress: (completed, total),
                    });
                }
            }
        }
    }
}
```

**Step 2: Load specs on app init**

Update `src/tui/app.rs` App::new() to accept spec_dir:

```rust
    pub fn new(spec_dir: &str) -> Self {
        let mut dashboard = DashboardState::new();
        dashboard.load_specs(spec_dir);

        Self {
            running: true,
            active_tab: ActiveTab::Dashboard,
            dashboard,
            planning: PlanningState::new(),
            execution: ExecutionState::new(),
            side_panel: SidePanel::new(),
        }
    }
```

**Step 3: Update main.rs to pass spec_dir**

Update the Tui command in main.rs:

```rust
        Commands::Tui => {
            let config_path = find_config_path()?;
            let config = config::Config::load(&config_path)?;
            let spec_dir = config.rustagent.spec_dir.clone();

            use rustagent::tui::{self, App};
            use crossterm::event::{self, Event, KeyEventKind};

            let mut terminal = tui::setup_terminal()?;
            let mut app = App::new(&spec_dir);
            // ... rest unchanged
        }
```

**Step 4: Verify it compiles and runs**

Run: `cargo check`
Expected: Compiles without errors

Run: `cargo run -- tui`
Expected: Dashboard shows specs from spec_dir in Kanban columns

**Step 5: Commit**

```bash
git add src/tui/ src/main.rs
git commit -m "feat(tui): load specs from disk in dashboard"
```

---

## Summary

This plan covers the core TUI implementation:

1. ✅ Dependencies
2. ✅ Module skeleton
3. ✅ Tab bar widget
4. ✅ Basic UI rendering
5. ✅ CLI command
6. ✅ Dashboard with Kanban/Activity
7. ✅ Planning view with chat
8. ✅ Execution view with split layout
9. ✅ Side panel
10. ✅ Spec loading

**Future tasks (not in this plan):**
- Async channel integration with PlanningAgent
- Async channel integration with RalphLoop
- Spinner animation
- Help overlay
- Keyboard navigation in Kanban
