use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::tui::{App, ActiveTab};
use crate::tui::views::draw_dashboard;
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
    match app.active_tab {
        ActiveTab::Dashboard => {
            draw_dashboard(frame, chunks[1], &app.dashboard);
        }
        _ => {
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
