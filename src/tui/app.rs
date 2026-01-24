use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::views::{DashboardMode, DashboardState, ExecutionState, MessageRole, PlanningState};
use crate::tui::widgets::SidePanel;

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
}

impl App {
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

    /// Handle key events. Uses full KeyEvent to preserve modifiers.
    pub fn handle_key(&mut self, key: KeyEvent) {
        // Handle planning insert mode separately
        if self.active_tab == ActiveTab::Planning && self.planning.insert_mode {
            match key.code {
                KeyCode::Esc => {
                    self.planning.insert_mode = false;
                }
                KeyCode::Enter => {
                    if let Some(text) = self.planning.submit_input() {
                        self.planning.add_message(MessageRole::User, text);
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
            (KeyCode::Char('k'), KeyModifiers::NONE) if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Kanban;
            }
            (KeyCode::Char('a'), KeyModifiers::NONE) if self.active_tab == ActiveTab::Dashboard => {
                self.dashboard.mode = DashboardMode::Activity;
            }
            (KeyCode::Char('i'), KeyModifiers::NONE) if self.active_tab == ActiveTab::Planning => {
                self.planning.insert_mode = true;
            }
            (KeyCode::Char('['), KeyModifiers::NONE) | (KeyCode::Char(']'), KeyModifiers::NONE) => {
                self.side_panel.toggle();
            }
            (KeyCode::Esc, _) => {
                if self.side_panel.visible {
                    self.side_panel.visible = false;
                }
            }
            _ => {}
        }
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new("")
    }
}
