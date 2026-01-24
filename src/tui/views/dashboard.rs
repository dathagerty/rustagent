use std::fs;
use std::path::Path;

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph},
};

use crate::spec::Spec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DashboardMode {
    Kanban,
    Activity,
}

pub struct DashboardState {
    pub mode: DashboardMode,
    pub specs: Vec<SpecSummary>,
    pub selected_column: usize,
    pub selected_row: usize,
}

#[derive(Debug, Clone)]
pub struct SpecSummary {
    pub name: String,
    pub status: SpecStatus,
    pub task_progress: (usize, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecStatus {
    Draft,
    Ready,
    Running,
    Completed,
}

impl DashboardState {
    pub fn new() -> Self {
        Self {
            mode: DashboardMode::Kanban,
            specs: Vec::new(),
            selected_column: 0,
            selected_row: 0,
        }
    }

    pub fn load_specs(&mut self, spec_dir: &str) {
        self.specs.clear();

        let spec_path = Path::new(spec_dir);
        if !spec_path.exists() {
            return;
        }

        if let Ok(entries) = fs::read_dir(spec_path) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Look for spec.json files
                let spec_file = if path.is_dir() {
                    path.join("spec.json")
                } else if path.extension().is_some_and(|e| e == "json") {
                    path
                } else {
                    continue;
                };

                if let Ok(spec) = Spec::load(&spec_file) {
                    let completed = spec
                        .tasks
                        .iter()
                        .filter(|t| t.status == crate::spec::TaskStatus::Complete)
                        .count();
                    let total = spec.tasks.len();

                    let status = if completed == total && total > 0 {
                        SpecStatus::Completed
                    } else if spec
                        .tasks
                        .iter()
                        .any(|t| t.status == crate::spec::TaskStatus::InProgress)
                    {
                        SpecStatus::Running
                    } else if total > 0 {
                        SpecStatus::Ready
                    } else {
                        SpecStatus::Draft
                    };

                    self.specs.push(SpecSummary {
                        name: spec.name,
                        status,
                        task_progress: (completed, total),
                    });
                }
            }
        }
    }
}

impl Default for DashboardState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn draw_dashboard(frame: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
        ])
        .split(area);

    let mode_text = match state.mode {
        DashboardMode::Kanban => " View: [K]anban │ Activity ",
        DashboardMode::Activity => " View: Kanban │ [A]ctivity ",
    };
    let mode_bar = Paragraph::new(mode_text)
        .style(Style::default().fg(Color::Cyan));
    frame.render_widget(mode_bar, chunks[0]);

    match state.mode {
        DashboardMode::Kanban => draw_kanban(frame, chunks[1], state),
        DashboardMode::Activity => draw_activity(frame, chunks[1], state),
    }
}

fn draw_kanban(frame: &mut ratatui::Frame, area: Rect, state: &DashboardState) {
    let columns = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
            Constraint::Percentage(25),
        ])
        .split(area);

    let column_titles = ["Draft", "Ready", "Running", "Completed"];
    let statuses = [SpecStatus::Draft, SpecStatus::Ready, SpecStatus::Running, SpecStatus::Completed];

    for (i, (col_area, (title, status))) in columns.iter()
        .zip(column_titles.iter().zip(statuses.iter()))
        .enumerate()
    {
        let is_selected = i == state.selected_column;
        let style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(*title)
            .border_style(style);

        let specs_in_column: Vec<&SpecSummary> = state.specs
            .iter()
            .filter(|s| s.status == *status)
            .collect();

        let items: Vec<ListItem> = specs_in_column
            .iter()
            .enumerate()
            .map(|(j, spec)| {
                let content = format!("{} ({}/{})", spec.name, spec.task_progress.0, spec.task_progress.1);
                let item_style = if is_selected && j == state.selected_row {
                    Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };
                ListItem::new(content).style(item_style)
            })
            .collect();

        let list = List::new(items).block(block);
        frame.render_widget(list, *col_area);
    }
}

fn draw_activity(frame: &mut ratatui::Frame, area: Rect, _state: &DashboardState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Activity Feed ");

    let header = Line::from(vec![
        Span::styled("Time      ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Event             ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Spec         ", Style::default().add_modifier(Modifier::BOLD)),
        Span::styled("│ ", Style::default().fg(Color::DarkGray)),
        Span::styled("Details", Style::default().add_modifier(Modifier::BOLD)),
    ]);

    let content = Paragraph::new(vec![
        header,
        Line::from("─".repeat(area.width.saturating_sub(2) as usize)),
        Line::from("  No activity yet"),
    ])
    .block(block);

    frame.render_widget(content, area);
}
