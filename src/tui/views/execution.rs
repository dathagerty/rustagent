use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
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
    pub auto_scroll: bool,
    pub scroll_to_bottom_pending: bool,
    pub content_height: usize,
    pub viewport_height: usize,
    pub last_area_width: u16,
}

impl ExecutionState {
    pub fn new() -> Self {
        Self {
            running: false,
            current_task: None,
            tasks: Vec::new(),
            output: Vec::new(),
            scroll_offset: 0,
            auto_scroll: true,
            scroll_to_bottom_pending: false,
            content_height: 0,
            viewport_height: 20,
            last_area_width: 0,
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
            // Note: scroll_offset will be recalculated on next render
            // based on actual content height, so this removal is handled
        }
        if self.auto_scroll {
            // Set flag to scroll on next render
            self.scroll_to_bottom_pending = true;
        }
    }

    pub fn clamp_scroll(&mut self, content_height: usize, viewport_height: usize) {
        let max_scroll = content_height.saturating_sub(viewport_height);
        self.scroll_offset = self.scroll_offset.min(max_scroll);
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

impl Default for ExecutionState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_execution(
    frame: &mut Frame,
    area: Rect,
    state: &mut ExecutionState,
    spinner_char: char,
) {
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

fn draw_output_pane(frame: &mut Frame, area: Rect, state: &mut ExecutionState) {
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
                        tc.name.clone(),
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

    // Update state and handle pending scroll
    state.content_height = content_height;
    state.viewport_height = viewport_height;
    state.last_area_width = inner.width;

    if state.scroll_to_bottom_pending {
        state.scroll_offset = content_height.saturating_sub(viewport_height);
        state.scroll_to_bottom_pending = false;
    }

    // Clamp scroll
    state.clamp_scroll(content_height, viewport_height);

    let scroll_y = state.scroll_offset.min(u16::MAX as usize) as u16;

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_y, 0));

    frame.render_widget(paragraph, inner);

    // Show scroll indicator if content is scrollable
    if content_height > viewport_height {
        let max_scroll = content_height.saturating_sub(viewport_height);
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
