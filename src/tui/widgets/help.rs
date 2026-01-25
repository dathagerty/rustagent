use ratatui::{
    layout::Rect,
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
        Line::from("  Shift+K/A  Kanban/Activity view"),
        Line::from("  ↑↓←→/hjkl Navigate"),
        Line::from("  Enter      Run selected spec"),
        Line::from(""),
        Line::from(Span::styled("Planning", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  i          Enter insert mode"),
        Line::from("  Esc        Exit insert mode"),
        Line::from("  Enter      Send message"),
        Line::from("  ↑↓/jk      Scroll (not in insert)"),
        Line::from("  PgUp/PgDn  Page scroll"),
        Line::from("  Home/End   Jump to top/bottom"),
        Line::from(""),
        Line::from(Span::styled("Execution", Style::default().add_modifier(Modifier::BOLD))),
        Line::from("  ↑↓/jk      Scroll output"),
        Line::from("  PgUp/PgDn  Page scroll"),
        Line::from("  Home/End   Jump to top/bottom"),
    ];

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, help_area);
}
