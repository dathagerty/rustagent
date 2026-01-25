use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::config::Config;
use crate::tui::messages::{AgentMessage, AgentSender};
use crate::tui::views::{
    DashboardMode, DashboardState, ExecutionState, MessageRole, NavDirection, OutputItem,
    PlanningState, ToolCall,
};
use crate::tui::widgets::{HelpOverlay, SidePanel, Spinner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveTab {
    Dashboard,
    Planning,
    Execution,
}

pub struct App {
    pub running: bool,
    pub active_tab: ActiveTab,
    pub dashboard: DashboardState,
    pub planning: PlanningState,
    pub execution: ExecutionState,
    pub side_panel: SidePanel,
    pub spec_dir: String,
    pub agent_tx: AgentSender,
    pub config: Option<Config>,
    pub spinner: Spinner,
    pub help: HelpOverlay,
    pub planning_active: bool,
    pub execution_active: bool,
}

impl App {
    pub fn new(spec_dir: &str, agent_tx: AgentSender, config: Option<Config>) -> Self {
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
            config,
            spinner: Spinner::new(),
            help: HelpOverlay::new(),
            planning_active: false,
            execution_active: false,
        }
    }

    /// Handle key events. Uses full KeyEvent to preserve modifiers.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Help overlay is modal - only Esc closes it
        if self.help.visible {
            if key.code == KeyCode::Esc {
                self.help.visible = false;
            }
            return;
        }

        // Handle planning insert mode separately
        if self.active_tab == ActiveTab::Planning && self.planning.insert_mode {
            match key.code {
                KeyCode::Esc => {
                    self.planning.insert_mode = false;
                }
                KeyCode::Enter => {
                    if let Some(text) = self.planning.submit_input() {
                        self.planning.add_message(MessageRole::User, text.clone());

                        // Don't spawn if already running
                        if self.planning.thinking {
                            return;
                        }

                        // Spawn planning agent if we have config
                        if let Some(ref config) = self.config {
                            self.planning.thinking = true;
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
                _ => {
                    self.planning.input.input(key);
                }
            }
            return;
        }

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
            (KeyCode::Char('K'), KeyModifiers::SHIFT) if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Kanban;
            }
            (KeyCode::Char('A'), KeyModifiers::SHIFT) if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Activity;
            }
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard =>
            {
                self.dashboard.move_selection(NavDirection::Up);
            }
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard =>
            {
                self.dashboard.move_selection(NavDirection::Down);
            }
            (KeyCode::Left, KeyModifiers::NONE) | (KeyCode::Char('h'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard =>
            {
                self.dashboard.move_selection(NavDirection::Left);
            }
            (KeyCode::Right, KeyModifiers::NONE) | (KeyCode::Char('l'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Dashboard =>
            {
                self.dashboard.move_selection(NavDirection::Right);
            }
            (KeyCode::Enter, KeyModifiers::NONE) if self.active_tab == ActiveTab::Dashboard => {
                // Don't spawn if execution already running
                if self.execution.running {
                    return;
                }

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
            (KeyCode::Char('i'), KeyModifiers::NONE) if self.active_tab == ActiveTab::Planning => {
                self.planning.insert_mode = true;
            }
            // Planning tab scrolling (not in insert mode)
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                self.planning.scroll_up(1);
            }
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                self.planning.scroll_down(1);
            }
            (KeyCode::PageUp, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                let page_size = self.planning.viewport_height.saturating_sub(1).max(1);
                self.planning.scroll_up(page_size);
            }
            (KeyCode::PageDown, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                let page_size = self.planning.viewport_height.saturating_sub(1).max(1);
                self.planning.scroll_down(page_size);
            }
            (KeyCode::Home, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                self.planning.scroll_offset = 0;
                self.planning.auto_scroll = false;
            }
            (KeyCode::End, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Planning && !self.planning.insert_mode =>
            {
                self.planning.scroll_to_bottom();
            }
            // Execution tab scrolling
            (KeyCode::Up, KeyModifiers::NONE) | (KeyCode::Char('k'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                self.execution.scroll_up(1);
            }
            (KeyCode::Down, KeyModifiers::NONE) | (KeyCode::Char('j'), KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                self.execution.scroll_down(1);
            }
            (KeyCode::PageUp, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                let page_size = self.execution.viewport_height.saturating_sub(1).max(1);
                self.execution.scroll_up(page_size);
            }
            (KeyCode::PageDown, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                let page_size = self.execution.viewport_height.saturating_sub(1).max(1);
                self.execution.scroll_down(page_size);
            }
            (KeyCode::Home, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                self.execution.scroll_offset = 0;
                self.execution.auto_scroll = false;
            }
            (KeyCode::End, KeyModifiers::NONE)
                if self.active_tab == ActiveTab::Execution =>
            {
                self.execution.scroll_to_bottom();
            }
            (KeyCode::Char('['), KeyModifiers::NONE) | (KeyCode::Char(']'), KeyModifiers::NONE) => {
                self.side_panel.toggle();
            }
            (KeyCode::Char('?'), KeyModifiers::NONE) => {
                self.help.toggle();
            }
            (KeyCode::Esc, _) => {
                if self.side_panel.visible {
                    self.side_panel.visible = false;
                }
            }
            _ => {}
        }
    }

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
            AgentMessage::PlanningToolCall { name, args: _ } => {
                self.planning.add_message(
                    MessageRole::Assistant,
                    format!("[Calling tool: {}]", name),
                );
            }
            AgentMessage::PlanningToolResult { name, output } => {
                let preview = if output.len() > 100 {
                    format!("{}...", &output[..100])
                } else {
                    output
                };
                self.planning.add_message(
                    MessageRole::Assistant,
                    format!("[{} result: {}]", name, preview),
                );
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
}

impl Default for App {
    fn default() -> Self {
        let (tx, _) = crate::tui::messages::agent_channel();
        Self::new("", tx, None)
    }
}
