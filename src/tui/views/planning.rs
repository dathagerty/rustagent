use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
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
    pub auto_scroll: bool,
    pub scroll_to_bottom_pending: bool,
    pub content_height: usize,
    pub viewport_height: usize,
    pub last_area_width: u16,
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
            auto_scroll: true,
            scroll_to_bottom_pending: false,
            content_height: 0,
            viewport_height: 20,
            last_area_width: 0,
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        self.messages.push(ChatMessage { role, content });
        if self.auto_scroll {
            // Set flag to scroll on next render
            self.scroll_to_bottom_pending = true;
        }
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

    pub fn scroll_up(&mut self, lines: usize) {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        // Disable auto-scroll when scrolling up from bottom
        let near_bottom_threshold = 3;
        if self.scroll_offset + near_bottom_threshold < self.max_scroll() {
            self.auto_scroll = false;
        }
    }

    pub fn scroll_down(&mut self, lines: usize) {
        let max_scroll = self.max_scroll();
        self.scroll_offset = (self.scroll_offset + lines).min(max_scroll);
        // Re-enable auto-scroll when near bottom (within 3 lines)
        let near_bottom_threshold = 3;
        if self.scroll_offset + near_bottom_threshold >= max_scroll {
            self.auto_scroll = true;
        }
    }

    pub fn max_scroll(&self) -> usize {
        // Guard against zero viewport height
        if self.viewport_height == 0 {
            return 0;
        }
        self.content_height.saturating_sub(self.viewport_height)
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = self.max_scroll();
        self.auto_scroll = true;
    }
}

impl Default for PlanningState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_planning(frame: &mut Frame, area: Rect, state: &mut PlanningState, spinner_char: char) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_chat_history(frame, chunks[0], state, spinner_char);

    let input_block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.insert_mode {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
        })
        .title(if state.insert_mode {
            " Input (INSERT) "
        } else {
            " Input "
        });

    state.input.set_block(input_block);
    frame.render_widget(&state.input, chunks[1]);

    let hints = if state.insert_mode {
        " Esc exit insert │ Enter send "
    } else {
        " i insert │ ↑↓ scroll │ Ctrl+S save │ ] spec panel "
    };
    let hints_bar = Paragraph::new(hints).style(Style::default().fg(Color::DarkGray));
    frame.render_widget(hints_bar, chunks[2]);
}

fn draw_chat_history(frame: &mut Frame, area: Rect, state: &mut PlanningState, spinner_char: char) {
    let block = Block::default().borders(Borders::ALL).title(" Chat ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.messages {
        let (label, style) = match msg.role {
            MessageRole::User => (
                "You",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            MessageRole::Assistant => (
                "Assistant",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        };

        lines.push(Line::from(Span::styled(label, style)));

        for line in msg.content.lines() {
            lines.push(Line::from(format!("  {}", line)));
        }
        lines.push(Line::from(""));
    }

    if state.thinking {
        lines.push(Line::from(vec![
            Span::styled(
                "Assistant",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(spinner_char.to_string(), Style::default().fg(Color::Yellow)),
        ]));
    }

    // Calculate actual wrapped content height
    let viewport_height = inner.height as usize;
    let viewport_width = inner.width as usize;

    // Calculate wrapped line count
    let mut content_height = 0;
    for line in &lines {
        let line_width = line.width();
        if line_width == 0 {
            content_height += 1;
        } else {
            // Account for wrapping
            content_height += (line_width + viewport_width - 1) / viewport_width.max(1);
        }
    }

    // Update state dimensions
    state.content_height = content_height;
    state.viewport_height = viewport_height;
    state.last_area_width = inner.width;

    // Handle pending scroll to bottom
    if state.scroll_to_bottom_pending {
        state.scroll_offset = content_height.saturating_sub(viewport_height);
        state.scroll_to_bottom_pending = false;
    }

    // Clamp scroll to valid range
    let max_scroll = if viewport_height == 0 {
        0
    } else {
        content_height.saturating_sub(viewport_height)
    };
    state.scroll_offset = state.scroll_offset.min(max_scroll);

    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);

    // Show scroll indicator if content is scrollable
    if content_height > viewport_height {
        let scroll_pct = if max_scroll > 0 {
            (state.scroll_offset * 100) / max_scroll
        } else {
            100
        };

        // Add [auto] indicator if auto-scroll is enabled
        let indicator = if state.auto_scroll {
            format!(" {}% [auto] ", scroll_pct)
        } else {
            format!(" {}% ", scroll_pct)
        };

        let indicator_width = indicator.len() as u16;
        let indicator_area = Rect::new(
            inner.x + inner.width.saturating_sub(indicator_width),
            inner.y + inner.height.saturating_sub(1),
            indicator_width,
            1,
        );

        let indicator_widget =
            Paragraph::new(indicator).style(Style::default().bg(Color::DarkGray).fg(Color::White));

        frame.render_widget(indicator_widget, indicator_area);
    }
}
