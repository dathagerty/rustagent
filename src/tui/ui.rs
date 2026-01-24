use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{App, ActiveTab};
use crate::tui::views::{draw_dashboard, draw_planning};
use crate::tui::widgets::TabBar;

pub fn draw(frame: &mut Frame, app: &mut App) {
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
    match app.active_tab {
        ActiveTab::Dashboard => {
            draw_dashboard(frame, chunks[1], &app.dashboard);
        }
        ActiveTab::Planning => {
            draw_planning(frame, chunks[1], &mut app.planning);
        }
        ActiveTab::Execution => {
            let content_block = Block::default()
                .borders(Borders::ALL)
                .title(" Execution ");

            let placeholder = Paragraph::new("Execution view - coming soon")
                .block(content_block);

            frame.render_widget(placeholder, chunks[1]);
        }
    }

    // Status bar
    let status = Line::from(vec![
        Span::raw(" q quit │ 1/2/3 switch tabs │ Tab cycle │ ? help "),
    ]);
    let status_bar = Paragraph::new(status)
        .style(Style::default().bg(Color::DarkGray));
    frame.render_widget(status_bar, chunks[2]);
}
