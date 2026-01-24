use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};

use crate::spec::{Task, TaskStatus};

#[derive(Debug, Clone)]
pub struct ToolCall {
    pub name: String,
    pub output: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone)]
pub enum OutputItem {
    Message { role: String, content: String },
    ToolCall(ToolCall),
}

const MAX_OUTPUT_ITEMS: usize = 1000;

pub struct ExecutionState {
    pub running: bool,
    pub current_task: Option<Task>,
    pub tasks: Vec<Task>,
    pub output: Vec<OutputItem>,
    pub scroll_offset: usize,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            running: false,
            current_task: None,
            tasks: Vec::new(),
            output: Vec::new(),
            scroll_offset: 0,
        }
    }

    pub fn task_progress(&self) -> (usize, usize) {
        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Complete)
            .count();
        (completed, self.tasks.len())
    }

    pub fn add_output(&mut self, item: OutputItem) {
        self.output.push(item);
        if self.output.len() > MAX_OUTPUT_ITEMS {
            self.output.remove(0);
            self.scroll_offset = self.scroll_offset.saturating_sub(1);
        }
    }

    pub fn clamp_scroll(&mut self, content_height: usize, viewport_height: usize) {
        let max_scroll = content_height.saturating_sub(viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
    }
}

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_execution(frame: &mut Frame, area: Rect, state: &ExecutionState, spinner_char: char) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)])
        .split(area);

    draw_task_pane(frame, chunks[0], state, spinner_char);
    draw_output_pane(frame, chunks[1], state);
}

fn draw_task_pane(frame: &mut Frame, area: Rect, state: &ExecutionState, spinner_char: char) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    let (completed, total) = state.task_progress();
    let title = if state.running {
        format!(" Current Task {}/{} {} ", completed, total, spinner_char)
    } else {
        format!(" Current Task {}/{} ", completed, total)
    };
    let current_block = Block::default().borders(Borders::ALL).title(title);

    let current_content = if let Some(task) = &state.current_task {
        let mut lines = vec![
            Line::from(Span::styled(
                format!("▶ {}", task.title),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "Acceptance Criteria:",
                Style::default().fg(Color::Cyan),
            )),
        ];
        for criterion in &task.acceptance_criteria {
            lines.push(Line::from(format!(" ☐ {}", criterion)));
        }
        lines
    } else {
        vec![Line::from("No task running")]
    };

    let current = Paragraph::new(current_content)
        .block(current_block)
        .wrap(Wrap { trim: false });
    frame.render_widget(current, chunks[0]);

    let tasks_block = Block::default().borders(Borders::ALL).title(" Tasks ");

    let items: Vec<ListItem> = state
        .tasks
        .iter()
        .map(|task| {
            let (icon, style) = match task.status {
                TaskStatus::Complete => ("✓", Style::default().fg(Color::Green)),
                TaskStatus::InProgress => ("▶", Style::default().fg(Color::Yellow)),
                TaskStatus::Pending => ("○", Style::default().fg(Color::DarkGray)),
                TaskStatus::Blocked => ("✗", Style::default().fg(Color::Red)),
            };
            ListItem::new(format!(" {} {}", icon, task.title)).style(style)
        })
        .collect();

    let list = List::new(items).block(tasks_block);
    frame.render_widget(list, chunks[1]);
}

fn draw_output_pane(frame: &mut Frame, area: Rect, state: &ExecutionState) {
    let block = Block::default().borders(Borders::ALL).title(" Output ");

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines: Vec<Line> = Vec::new();

    for item in &state.output {
        match item {
            OutputItem::Message { role, content } => {
                let style = if role == "Assistant" {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD)
                };
                lines.push(Line::from(Span::styled(role.clone(), style)));
                for line in content.lines() {
                    lines.push(Line::from(format!("  {}", line)));
                }
                lines.push(Line::from(""));
            }
            OutputItem::ToolCall(tc) => {
                lines.push(Line::from(vec![
                    Span::raw("┌─ "),
                    Span::styled(
                        &tc.name,
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ─"),
                ]));
                if !tc.collapsed {
                    for line in tc.output.lines().take(5) {
                        lines.push(Line::from(format!("│ {}", line)));
                    }
                }
                lines.push(Line::from("└────────────────────"));
                lines.push(Line::from(""));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from("  Waiting for execution..."));
    }

    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);
}
