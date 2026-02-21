use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::tui::app::{App, FocusedPanel, LogEntry, TrainingEntry};

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_task_info(frame, outer[0], app);

    let content = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(18), Constraint::Min(20)])
        .split(outer[1]);

    render_sidebar(frame, content[0], app);

    render_active_panel(frame, content[1], app);
    render_status_bar(frame, outer[2], app);
}

fn render_task_info(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
    let task = app.task.as_ref();
    let status = task
        .map(|task| format!("{:?}", task.status))
        .unwrap_or_else(|| "Unknown".to_string());
    let description = task
        .map(|task| task.task_description.clone())
        .unwrap_or_else(|| "-".to_string());
    let dataset = task
        .map(|task| task.dataset_path.clone())
        .unwrap_or_else(|| "-".to_string());
    let created = task
        .map(|task| format_timestamp(Some(task.created_at)))
        .unwrap_or_else(|| "-".to_string());
    let started = task
        .and_then(|task| task.started_at)
        .map(|ts| format_timestamp(Some(ts)))
        .unwrap_or_else(|| "-".to_string());
    let completed = task
        .and_then(|task| task.completed_at)
        .map(|ts| format_timestamp(Some(ts)))
        .unwrap_or_else(|| match task.map(|task| task.status) {
            Some(mindbox_common::TaskStatus::Running) => "In progress".to_string(),
            Some(mindbox_common::TaskStatus::Pending) => "Not started".to_string(),
            _ => "N/A".to_string(),
        });

    let lines = vec![
        Line::from(vec![
            Span::styled("ID: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(app.task_id.as_str()),
            Span::raw("   "),
            Span::styled("Status: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(status),
        ]),
        Line::from(vec![
            Span::styled(
                "Description: ",
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(description),
        ]),
        Line::from(vec![
            Span::styled("Dataset: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(dataset),
        ]),
        Line::from(vec![
            Span::styled("Created: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(created),
        ]),
        Line::from(vec![
            Span::styled("Started: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(started),
        ]),
        Line::from(vec![
            Span::styled("Completed: ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(completed),
        ]),
    ];

    let widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Task Info"));
    frame.render_widget(widget, area);
}

fn render_sidebar(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
    let kernel_style = if app.focused == FocusedPanel::Kernel {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let training_style = if app.focused == FocusedPanel::Training {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    let lines = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(
                if app.focused == FocusedPanel::Kernel {
                    "> "
                } else {
                    "  "
                },
                kernel_style,
            ),
            Span::styled("Kernel Output", kernel_style),
        ]),
        Line::from(vec![
            Span::styled(
                if app.focused == FocusedPanel::Training {
                    "> "
                } else {
                    "  "
                },
                training_style,
            ),
            Span::styled("Training Logs", training_style),
        ]),
    ];

    let widget = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

fn render_active_panel(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &mut App) {
    let (lines, offset, auto_scroll) = match app.focused {
        FocusedPanel::Kernel => (
            kernel_lines(
                &app.kernel_logs,
                app.expand_thinking,
                app.expand_tool_results,
            ),
            app.kernel_scroll.offset,
            app.kernel_scroll.auto_scroll,
        ),
        FocusedPanel::Training => (
            training_lines(&app.training_logs),
            app.training_scroll.offset,
            app.training_scroll.auto_scroll,
        ),
    };

    let content_width = area.width.saturating_sub(2).max(1) as usize;
    let total_visual_lines = visual_line_count(&lines, content_width);
    let viewport = area.height.saturating_sub(2) as usize;
    let max_offset = total_visual_lines.saturating_sub(viewport);
    app.set_panel_max_offset(app.focused, max_offset);
    let scroll = effective_scroll(offset, auto_scroll, total_visual_lines, area.height);

    let widget = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(panel_border_style()),
        );
    frame.render_widget(widget, area);
}

fn render_status_bar(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
    let text = match app.focused {
        FocusedPanel::Kernel => {
            let thinking_state = if app.expand_thinking {
                "expanded".to_string()
            } else {
                "collapsed".to_string()
            };
            let tool_use_state = if app.expand_tool_results {
                "expanded".to_string()
            } else {
                "collapsed".to_string()
            };
            format!(
                "{} | Switch Panel(tab): Kernel Output | Thinking(t): {} | Tool Use(o): {}",
                app.connection_status, thinking_state, tool_use_state
            )
        }
        FocusedPanel::Training => {
            format!(
                "{} | Switch Panel(tab): Training Logs",
                app.connection_status
            )
        }
    };

    let widget = Paragraph::new(text).style(Style::default().fg(Color::Black).bg(Color::Gray));
    frame.render_widget(widget, area);
}

fn effective_scroll(offset: usize, auto_scroll: bool, total_lines: usize, height: u16) -> u16 {
    let viewport = height.saturating_sub(2) as usize;
    let max_offset = total_lines.saturating_sub(viewport);
    let selected = if auto_scroll {
        max_offset
    } else {
        offset.min(max_offset)
    };
    selected.min(u16::MAX as usize) as u16
}

fn kernel_lines(
    entries: &[LogEntry],
    expand_thinking: bool,
    expand_tool_results: bool,
) -> Vec<Line<'static>> {
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut out = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            let prev = &entries[i - 1];
            let tool_pair_contiguous = matches!(prev, LogEntry::ToolUse { .. })
                && matches!(entry, LogEntry::ToolResult(_));
            if !tool_pair_contiguous {
                out.push(Line::from(""));
            }
        }

        match entry {
            LogEntry::AssistantText(text) => {
                push_multiline(&mut out, text, Style::default().fg(Color::White), "");
            }
            LogEntry::Thinking(text) => {
                let line_count = text.lines().count().max(1);
                out.push(Line::from(vec![Span::styled(
                    "Thinking:",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )]));
                if expand_thinking {
                    push_multiline(&mut out, text, Style::default().fg(Color::DarkGray), "  ");
                } else {
                    out.push(Line::from(vec![Span::styled(
                        format!("  ({line_count} lines) [collapsed]"),
                        Style::default()
                            .fg(Color::Magenta)
                            .add_modifier(Modifier::DIM),
                    )]));
                }
            }
            LogEntry::ToolUse { name, summary, .. } => {
                out.push(Line::from(vec![Span::styled(
                    format!("Tool Use: {name}"),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                )]));
                if !summary.is_empty() {
                    out.push(Line::from(vec![Span::styled(
                        format!("  {summary}"),
                        dim_style,
                    )]));
                }
            }
            LogEntry::ToolResult(content) => {
                let is_error = content.starts_with("Error")
                    || content.starts_with("error")
                    || content.starts_with("ERROR");
                let line_count = content.lines().count().max(1);
                let attached_to_tool_use =
                    i > 0 && matches!(entries[i - 1], LogEntry::ToolUse { .. });
                let header_prefix = if attached_to_tool_use { "  " } else { "" };
                let body_prefix = if attached_to_tool_use { "    " } else { "  " };

                if !expand_tool_results {
                    let label = if is_error {
                        format!("{header_prefix}❌ ({line_count} lines) [collapsed]")
                    } else {
                        format!("{header_prefix}({line_count} lines) [collapsed]")
                    };
                    let style = if is_error {
                        Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
                    } else {
                        Style::default()
                            .fg(Color::Green)
                            .add_modifier(Modifier::DIM)
                    };
                    out.push(Line::from(vec![Span::styled(label, style)]));
                } else if is_error {
                    out.push(Line::from(vec![Span::styled(
                        format!("{header_prefix}❌"),
                        Style::default().fg(Color::Red).add_modifier(Modifier::DIM),
                    )]));
                }

                if expand_tool_results {
                    push_multiline(&mut out, content, dim_style, body_prefix);
                }
            }
            LogEntry::SystemMessage(message) => {
                out.push(Line::from(vec![
                    Span::styled(
                        "[system] ",
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(message.clone(), Style::default().fg(Color::Yellow)),
                ]));
            }
            LogEntry::ResultMessage(message) => {
                push_multiline(&mut out, message, Style::default().fg(Color::White), "");
            }
            LogEntry::Raw(line) => {
                push_multiline(&mut out, line, dim_style, "");
            }
        }
    }
    if out.is_empty() {
        out.push(Line::from("Waiting for kernel output..."));
    }
    out
}

fn training_lines(entries: &[TrainingEntry]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for entry in entries {
        match entry {
            TrainingEntry::Status { status, message } => out.push(Line::from(vec![
                Span::styled(
                    format!("[{status}] "),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(message.clone(), Style::default().fg(Color::White)),
            ])),
            TrainingEntry::Metric { name, value, step } => {
                let line = if let Some(step) = step {
                    format!("{name} = {value} step={step}")
                } else {
                    format!("{name} = {value}")
                };
                out.push(Line::from(vec![Span::styled(
                    line,
                    Style::default().fg(Color::Cyan),
                )]));
            }
            TrainingEntry::Error(message) => out.push(Line::from(vec![
                Span::styled(
                    "[error] ",
                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                ),
                Span::styled(message.clone(), Style::default().fg(Color::White)),
            ])),
            TrainingEntry::Log(line) => {
                push_multiline(&mut out, line, Style::default().fg(Color::White), "");
            }
        }
    }
    if out.is_empty() {
        out.push(Line::from("Waiting for training output..."));
    }
    out
}

fn push_multiline(lines: &mut Vec<Line<'static>>, text: &str, style: Style, prefix: &str) {
    for line in text.lines() {
        lines.push(Line::from(vec![Span::styled(
            format!("{prefix}{line}"),
            style,
        )]));
    }
    if text.is_empty() {
        lines.push(Line::from(vec![Span::styled(prefix.to_string(), style)]));
    }
}

fn format_timestamp(time: Option<DateTime<Utc>>) -> String {
    match time {
        Some(ts) => ts
            .with_timezone(&Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
        None => "N/A".to_string(),
    }
}

fn visual_line_count(lines: &[Line<'_>], content_width: usize) -> usize {
    lines
        .iter()
        .map(|line| {
            let width = line.width();
            width.max(1).div_ceil(content_width)
        })
        .sum()
}

fn panel_border_style() -> Style {
    Style::default().fg(Color::Yellow)
}
