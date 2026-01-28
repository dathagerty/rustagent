use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
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
