use chrono::{DateTime, Local, Utc};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::tui::app::{App, LogEntry, TOOL_RESULT_PREVIEW_TAIL};

pub fn draw(frame: &mut Frame<'_>, app: &mut App) {
    let outer = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(8),
            Constraint::Min(10),
            Constraint::Length(1),
        ])
        .split(frame.area());

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(outer[0]);

    render_task_info(frame, top[0], app);
    render_system_resources(frame, top[1], app);

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

fn render_system_resources(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title("System Resources");
    let inner = block.inner(area);
    frame.render_widget(block, area);

    if inner.height == 0 {
        return;
    }

    let Some(resources) = app.system_resources.as_ref() else {
        frame.render_widget(Paragraph::new("Waiting for data..."), inner);
        return;
    };

    let mut metrics = Vec::new();
    metrics.push((
        "CPU".to_string(),
        resources.cpu.utilization_percent,
        format!("{:.1}%", resources.cpu.utilization_percent),
    ));
    metrics.push((
        "MEM".to_string(),
        resources.memory.utilization_percent,
        format!(
            "{:.1}G/{:.1}G",
            bytes_to_gib(resources.memory.used_bytes),
            bytes_to_gib(resources.memory.total_bytes),
        ),
    ));

    let multi_gpu = resources.gpus.len() > 1;
    for (index, gpu) in resources.gpus.iter().enumerate() {
        let gpu_util_label = if multi_gpu {
            format!("GPU{index} UTIL")
        } else {
            "GPU UTIL".to_string()
        };
        let gpu_mem_label = if multi_gpu {
            format!("GPU{index} MEM")
        } else {
            "GPU MEM".to_string()
        };

        metrics.push((
            gpu_util_label,
            gpu.utilization_percent,
            format!("{:.1}%", gpu.utilization_percent),
        ));
        metrics.push((
            gpu_mem_label,
            gpu.memory_utilization_percent,
            format!(
                "{:.1}G/{:.1}G",
                bytes_to_gib(gpu.memory_used_bytes),
                bytes_to_gib(gpu.memory_total_bytes),
            ),
        ));
    }

    let visible_count = metrics.len().min(inner.height as usize);
    if visible_count == 0 {
        return;
    }
    let label_width = metrics
        .iter()
        .take(visible_count)
        .map(|(label, _, _)| label.chars().count())
        .max()
        .unwrap_or(0)
        .max(8);

    let top_padding = ((inner.height as usize).saturating_sub(visible_count) / 2) as u16;
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                vec![Constraint::Length(top_padding)],
                vec![Constraint::Length(1); visible_count],
                vec![Constraint::Min(0)],
            ]
            .concat(),
        )
        .split(inner);

    for (area, (label, percent, value)) in rows
        .iter()
        .skip(1)
        .take(visible_count)
        .zip(metrics.into_iter().take(visible_count))
    {
        if area.height == 0 {
            continue;
        }

        render_meter_line(frame, *area, &label, percent, &value, label_width);
    }
}

fn render_sidebar(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &App) {
    let mut lines = vec![Line::from("")];
    lines.push(sidebar_line("kernel.log", app.focused_index == 0));
    for (index, panel) in app.log_panels.iter().enumerate() {
        lines.push(sidebar_line(
            &panel.filename,
            app.focused_index == index + 1,
        ));
    }

    let widget = Paragraph::new(lines).block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

fn render_active_panel(frame: &mut Frame<'_>, area: ratatui::layout::Rect, app: &mut App) {
    let (lines, offset, auto_scroll) = match app.focused_index {
        0 => (
            kernel_lines(&app.kernel_logs, app.expand_all_results),
            app.kernel_scroll.offset,
            app.kernel_scroll.auto_scroll,
        ),
        index => match app.log_panels.get(index - 1) {
            Some(panel) => (
                plain_log_lines(&panel.lines),
                panel.scroll.offset,
                panel.scroll.auto_scroll,
            ),
            None => (plain_log_lines(&[]), 0, true),
        },
    };

    let content_width = area.width.saturating_sub(2).max(1) as usize;
    let total_visual_lines = visual_line_count(&lines, content_width);
    let viewport = area.height.saturating_sub(2) as usize;
    let max_offset = total_visual_lines.saturating_sub(viewport);
    app.set_panel_max_offset(app.focused_index, max_offset);
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
    let line = Line::from(format!(
        "{} | Quit: Esc/q | Scroll: ↑/↓/PgUp/PgDn/Home/End | Expand: Ctrl+o",
        app.connection_status
    ));

    let widget = Paragraph::new(line).style(Style::default().fg(Color::Black).bg(Color::Gray));
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

fn kernel_lines(entries: &[LogEntry], expand_all_results: bool) -> Vec<Line<'static>> {
    let dim_style = Style::default().fg(Color::DarkGray);

    let mut out = Vec::new();
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            let prev = &entries[i - 1];
            let tool_pair_contiguous = matches!(prev, LogEntry::ToolUse { .. })
                && matches!(entry, LogEntry::ToolResult { .. });
            if !tool_pair_contiguous {
                out.push(Line::from(""));
            }
        }

        match entry {
            LogEntry::AssistantText(text) => {
                push_multiline(&mut out, text, Style::default().fg(Color::White), "");
            }
            LogEntry::Thinking(text) => {
                out.push(Line::from(vec![Span::styled(
                    "Thinking:",
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                )]));
                push_multiline(&mut out, text, Style::default().fg(Color::DarkGray), "  ");
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
            LogEntry::ToolResult { content, .. } => {
                let lower = content.trim_start().to_ascii_lowercase();
                let is_error = lower.starts_with("error") || lower.contains("error:");
                let attached_to_tool_use =
                    i > 0 && matches!(entries[i - 1], LogEntry::ToolUse { .. });
                let first_prefix = if attached_to_tool_use {
                    "  ⎿ "
                } else {
                    "⎿ "
                };
                let body_prefix = if attached_to_tool_use { "    " } else { "  " };
                let style = if is_error {
                    Style::default().fg(Color::Red).add_modifier(Modifier::DIM)
                } else {
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::DIM)
                };

                let visible_lines = tool_result_visible_lines(content, expand_all_results);
                for (idx, line) in visible_lines.iter().enumerate() {
                    let prefix = if idx == 0 { first_prefix } else { body_prefix };
                    out.push(Line::from(vec![Span::styled(
                        format!("{prefix}{line}"),
                        style,
                    )]));
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
                out.push(Line::from(vec![Span::styled(
                    "━━━ Result ━━━",
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                )]));
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

fn tool_result_visible_lines(content: &str, expand_all_results: bool) -> Vec<String> {
    let lines: Vec<&str> = content.lines().collect();
    if lines.is_empty() {
        return vec![String::new()];
    }

    let total = lines.len();
    if expand_all_results || total <= TOOL_RESULT_PREVIEW_TAIL {
        return lines.into_iter().map(ToOwned::to_owned).collect();
    }

    let omitted = total - TOOL_RESULT_PREVIEW_TAIL;
    let mut out = Vec::with_capacity(1 + TOOL_RESULT_PREVIEW_TAIL);
    out.push(format!("... (省略前 {omitted} 行，ctrl+o 展开)"));
    out.extend(
        lines
            .iter()
            .skip(total - TOOL_RESULT_PREVIEW_TAIL)
            .map(|line| (*line).to_string()),
    );
    out
}

fn plain_log_lines(lines: &[String]) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for line in lines {
        push_multiline(&mut out, line, Style::default().fg(Color::White), "");
    }
    if out.is_empty() {
        out.push(Line::from("Waiting for output..."));
    }
    out
}

fn sidebar_line(label: &str, focused: bool) -> Line<'static> {
    let style = if focused {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };

    Line::from(vec![
        Span::styled(if focused { "> " } else { "  " }, style),
        Span::styled(label.to_string(), style),
    ])
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

fn usage_color(percent: f32) -> Color {
    if percent < 60.0 {
        Color::Green
    } else if percent <= 85.0 {
        Color::Yellow
    } else {
        Color::Red
    }
}

fn bytes_to_gib(bytes: u64) -> f64 {
    bytes as f64 / 1024_f64.powi(3)
}

fn render_meter_line(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    percent: f32,
    value: &str,
    label_width: usize,
) {
    let percent = percent.clamp(0.0, 100.0);
    let label_text = format!("{label:<label_width$}");
    let value_text = value.to_string();

    let total_width = area.width as usize;
    if total_width == 0 {
        return;
    }

    let reserved = label_text.chars().count() + 3;
    if total_width <= reserved {
        let fallback = Paragraph::new(label_text);
        frame.render_widget(fallback, area);
        return;
    }

    let content_width = total_width - reserved;
    let value_len = value_text.chars().count();
    if content_width <= value_len {
        let fallback = Paragraph::new(format!("{label_text} [{value_text}]"));
        frame.render_widget(fallback, area);
        return;
    }

    let gauge_width = content_width.saturating_sub(value_len);
    let filled = ((gauge_width as f32) * (percent / 100.0))
        .round()
        .clamp(0.0, gauge_width as f32) as usize;
    let empty = gauge_width.saturating_sub(filled);

    let line = Line::from(vec![
        Span::styled(
            label_text,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" ["),
        Span::styled(
            "|".repeat(filled),
            Style::default()
                .fg(usage_color(percent))
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("░".repeat(empty), Style::default().fg(Color::DarkGray)),
        Span::styled(
            value_text,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("]"),
    ]);
    frame.render_widget(Paragraph::new(line), area);
}
