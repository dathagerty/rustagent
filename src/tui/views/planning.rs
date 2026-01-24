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
            Constraint::Min(0),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(area);

    draw_chat_history(frame, chunks[0], state);

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

fn draw_chat_history(frame: &mut Frame, area: Rect, state: &PlanningState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Chat ");

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
            Span::styled("◐", Style::default().fg(Color::Yellow)),
        ]));
    }

    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
}
