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
