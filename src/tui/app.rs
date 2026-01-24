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
