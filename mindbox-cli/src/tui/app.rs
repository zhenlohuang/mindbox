use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use mindbox_common::{SystemResources, Task, TaskEvent, TaskStatus};
use serde_json::Value;

use crate::tui::event::AppEvent;

pub const TOOL_RESULT_PREVIEW_TAIL: usize = 10;

#[derive(Debug, Clone)]
pub enum LogEntry {
    AssistantText(String),
    Thinking(String),
    ToolUse {
        name: String,
        summary: String,
    },
    ToolResult {
        content: String,
        tool_use_id: Option<String>,
    },
    SystemMessage(String),
    ResultMessage(String),
    Raw(String),
}

#[derive(Debug, Clone, Copy)]
pub struct ScrollState {
    pub offset: usize,
    pub auto_scroll: bool,
}

impl Default for ScrollState {
    fn default() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }
}

pub struct LogPanel {
    pub filename: String,
    pub lines: Vec<String>,
    pub scroll: ScrollState,
    pub max_offset: usize,
}

pub struct App {
    pub task_id: String,
    pub task: Option<Task>,
    pub system_resources: Option<SystemResources>,
    pub kernel_logs: Vec<LogEntry>,
    pub log_panels: Vec<LogPanel>,
    pub focused_index: usize,
    pub kernel_scroll: ScrollState,
    pub connection_status: String,
    pub should_quit: bool,
    pub stream_ended: bool,
    pub expand_all_results: bool,
    pub kernel_max_offset: usize,
}

impl App {
    pub fn new(task_id: String) -> Self {
        Self {
            task_id,
            task: None,
            system_resources: None,
            kernel_logs: Vec::new(),
            log_panels: Vec::new(),
            focused_index: 0,
            kernel_scroll: ScrollState::default(),
            connection_status: "Connecting".to_string(),
            should_quit: false,
            stream_ended: false,
            expand_all_results: false,
            kernel_max_offset: 0,
        }
    }

    pub fn handle(&mut self, event: AppEvent) {
        match event {
            AppEvent::Key(key) => self.handle_key(key),
            AppEvent::Resize(width, height) => {
                let _ = (width, height);
            }
            AppEvent::ScrollUp => self.scroll_up(3),
            AppEvent::ScrollDown => self.scroll_down(3),
            AppEvent::TaskEvent(event) => self.handle_task_event(event),
            AppEvent::RawLog(line) => self.handle_raw_log(line),
            AppEvent::StreamConnected => {
                self.connection_status = "Connected".to_string();
            }
            AppEvent::StreamEnded => {
                self.stream_ended = true;
                self.connection_status = "Stream ended".to_string();
            }
            AppEvent::TaskInfo(task) => self.task = Some(*task),
            AppEvent::LogLine { filename, line } => self.push_to_panel(&filename, line),
            AppEvent::LogFileDiscovered(filename) => {
                let _ = self.find_or_create_panel(&filename);
            }
            AppEvent::Tick => {}
            AppEvent::SystemResources(resources) => self.system_resources = Some(*resources),
        }
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.should_quit = true;
            }
            KeyCode::Char('k') => self.scroll_up(3),
            KeyCode::Char('j') => self.scroll_down(3),
            KeyCode::Char('o') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.expand_all_results = !self.expand_all_results;
                if self.kernel_scroll.auto_scroll {
                    self.kernel_scroll.offset = self.kernel_total_lines();
                }
            }
            KeyCode::Tab => {
                self.focused_index = (self.focused_index + 1) % self.panel_count();
            }
            KeyCode::Up => self.scroll_up(3),
            KeyCode::Down => self.scroll_down(3),
            KeyCode::PageUp => self.scroll_up(12),
            KeyCode::PageDown => self.scroll_down(12),
            KeyCode::Home => self.scroll_to_top(),
            KeyCode::End => {
                let scroll = self.focused_scroll_mut();
                scroll.auto_scroll = true;
            }
            _ => {}
        }
    }

    fn focused_scroll_mut(&mut self) -> &mut ScrollState {
        if self.focused_index == 0 {
            &mut self.kernel_scroll
        } else {
            &mut self.log_panels[self.focused_index - 1].scroll
        }
    }

    fn scroll_up(&mut self, amount: usize) {
        let fallback_bottom = self.focused_max_offset();
        let scroll = self.focused_scroll_mut();
        if scroll.auto_scroll {
            scroll.auto_scroll = false;
            scroll.offset = fallback_bottom;
        }
        scroll.offset = scroll.offset.min(fallback_bottom).saturating_sub(amount);
    }

    fn scroll_down(&mut self, amount: usize) {
        let max_offset = self.focused_max_offset();
        let scroll = self.focused_scroll_mut();
        if scroll.auto_scroll {
            return;
        }
        scroll.offset = scroll.offset.saturating_add(amount).min(max_offset);
    }

    fn scroll_to_top(&mut self) {
        let scroll = self.focused_scroll_mut();
        scroll.auto_scroll = false;
        scroll.offset = 0;
    }

    fn focused_max_offset(&self) -> usize {
        if self.focused_index == 0 {
            self.kernel_max_offset
        } else {
            self.log_panels[self.focused_index - 1].max_offset
        }
    }

    pub fn set_panel_max_offset(&mut self, panel_index: usize, max_offset: usize) {
        if panel_index == 0 {
            self.kernel_max_offset = max_offset;
        } else if let Some(panel) = self.log_panels.get_mut(panel_index - 1) {
            panel.max_offset = max_offset;
        }
    }

    fn handle_task_event(&mut self, event: TaskEvent) {
        match event {
            TaskEvent::Log { message, .. } => self.route_log_message(message),
            TaskEvent::StatusUpdate {
                status, message, ..
            } => {
                self.push_to_panel(
                    "events.log",
                    format!("[status: {}] {}", format_status(status), message),
                );
                if matches!(
                    status,
                    TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
                ) {
                    self.stream_ended = true;
                    self.connection_status = "Task ended".to_string();
                }
            }
            TaskEvent::Metric { metric, .. } => {
                let line = if let Some(step) = metric.step {
                    format!("[metric] {} = {} step={step}", metric.name, metric.value)
                } else {
                    format!("[metric] {} = {}", metric.name, metric.value)
                };
                self.push_to_panel("events.log", line);
            }
            TaskEvent::Error { message, .. } => {
                self.push_to_panel("events.log", format!("[error] {message}"));
            }
        }
    }

    fn handle_raw_log(&mut self, line: String) {
        self.route_log_message(line);
    }

    fn route_log_message(&mut self, message: String) {
        let parsed = parse_claude_log_entries(&message);
        if !parsed.is_empty() {
            for entry in parsed {
                self.push_kernel(entry);
            }
            return;
        }

        if let Some(entry) = parse_jsonl_fallback_entry(&message) {
            self.push_kernel(entry);
            return;
        }

        let line = sanitize_line_for_display(&message);
        if !line.is_empty() {
            self.push_kernel(LogEntry::Raw(line));
        }
    }

    pub fn find_or_create_panel(&mut self, filename: &str) -> usize {
        if let Some(position) = self.log_panels.iter().position(|p| p.filename == filename) {
            return position + 1;
        }

        let insert_at = self
            .log_panels
            .partition_point(|panel| panel.filename.as_str() < filename);
        self.log_panels.insert(
            insert_at,
            LogPanel {
                filename: filename.to_string(),
                lines: Vec::new(),
                scroll: ScrollState::default(),
                max_offset: 0,
            },
        );

        let global_index = insert_at + 1;
        if self.focused_index > 0 && self.focused_index >= global_index {
            self.focused_index += 1;
        }

        global_index
    }

    pub fn push_to_panel(&mut self, filename: &str, line: String) {
        let panel_index = self.find_or_create_panel(filename);
        let line = sanitize_line_for_display(&line);
        if line.is_empty() {
            return;
        }
        if let Some(panel) = self.log_panels.get_mut(panel_index - 1) {
            panel.lines.push(line);
            if panel.scroll.auto_scroll {
                panel.scroll.offset = panel.lines.len();
            }
        }
    }

    pub fn panel_count(&self) -> usize {
        1 + self.log_panels.len()
    }

    fn push_kernel(&mut self, entry: LogEntry) {
        if let LogEntry::ToolResult {
            tool_use_id: Some(new_id),
            ..
        } = &entry
            && let Some(LogEntry::ToolResult {
                tool_use_id: Some(prev_id),
                ..
            }) = self.kernel_logs.last()
            && prev_id == new_id
        {
            let last = self.kernel_logs.len() - 1;
            self.kernel_logs[last] = entry;
        } else {
            self.kernel_logs.push(entry);
        }
        if self.kernel_scroll.auto_scroll {
            self.kernel_scroll.offset = self.kernel_total_lines();
        }
    }

    pub fn kernel_total_lines(&self) -> usize {
        let mut total = 0;
        for (i, entry) in self.kernel_logs.iter().enumerate() {
            if i > 0 {
                let prev = &self.kernel_logs[i - 1];
                let tool_pair_contiguous = matches!(prev, LogEntry::ToolUse { .. })
                    && matches!(entry, LogEntry::ToolResult { .. });
                if !tool_pair_contiguous {
                    total += 1;
                }
            }
            match entry {
                LogEntry::AssistantText(text) => total += text.lines().count().max(1),
                LogEntry::Thinking(text) => {
                    total += 1;
                    total += text.lines().count().max(1);
                }
                LogEntry::ToolUse { summary, .. } => {
                    total += 1;
                    if !summary.is_empty() {
                        total += 1;
                    }
                }
                LogEntry::ToolResult { content, .. } => {
                    let line_count = content.lines().count().max(1);
                    if self.expand_all_results || line_count <= TOOL_RESULT_PREVIEW_TAIL {
                        total += line_count;
                    } else {
                        total += 1 + TOOL_RESULT_PREVIEW_TAIL;
                    }
                }
                LogEntry::SystemMessage(_) => total += 1,
                LogEntry::ResultMessage(text) => total += 1 + text.lines().count().max(1),
                LogEntry::Raw(text) => total += text.lines().count().max(1),
            }
        }
        total
    }
}

fn extract_tool_summary(name: &str, input: Option<&Value>) -> String {
    let input = match input {
        Some(v) => v,
        None => return String::new(),
    };

    match name {
        "Read" | "Write" => input
            .get("file_path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "Edit" => input
            .get("file_path")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "Bash" => {
            let cmd = input.get("command").and_then(Value::as_str).unwrap_or("");
            if cmd.len() > 80 {
                format!("{}…", &cmd[..80])
            } else {
                cmd.to_string()
            }
        }
        "Grep" => {
            let pattern = input.get("pattern").and_then(Value::as_str).unwrap_or("");
            let path = input.get("path").and_then(Value::as_str).unwrap_or(".");
            format!("`{pattern}` in {path}")
        }
        "Glob" => input
            .get("pattern")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "WebSearch" => input
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        "WebFetch" => input
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        _ => {
            // First string field or empty
            if let Value::Object(map) = input {
                for v in map.values() {
                    if let Value::String(s) = v {
                        if !s.is_empty() {
                            let s = if s.len() > 80 {
                                format!("{}…", &s[..80])
                            } else {
                                s.clone()
                            };
                            return s;
                        }
                    }
                }
            }
            String::new()
        }
    }
}

fn parse_claude_log_entries(line: &str) -> Vec<LogEntry> {
    let value = match serde_json::from_str::<Value>(line) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };

    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    match event_type.as_str() {
        "assistant" => parse_assistant_message_entries(&value),
        "user" => parse_user_message_entries(&value),
        "tool_use" => {
            let name = value
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| value.get("tool_name").and_then(Value::as_str))
                .or_else(|| value.pointer("/tool/name").and_then(Value::as_str))
                .unwrap_or("unknown")
                .to_string();
            let input_value = value
                .get("input")
                .or_else(|| value.get("tool_input"))
                .or_else(|| value.get("arguments"));
            let summary = extract_tool_summary(&name, input_value);
            vec![LogEntry::ToolUse { name, summary }]
        }
        "tool_result" => {
            let content = value
                .get("content")
                .or_else(|| value.get("result"))
                .or_else(|| value.get("message"))
                .map(format_value)
                .or_else(|| extract_textish(Some(&value)));
            let tool_use_id = value
                .get("tool_use_id")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned);
            content
                .map(|content| LogEntry::ToolResult {
                    content,
                    tool_use_id,
                })
                .into_iter()
                .collect()
        }
        "system" => {
            let message = extract_textish(
                value
                    .get("message")
                    .or_else(|| value.get("text"))
                    .or_else(|| value.get("content")),
            );
            message.map(LogEntry::SystemMessage).into_iter().collect()
        }
        "result" => {
            let message = extract_textish(
                value
                    .get("text")
                    .or_else(|| value.get("result"))
                    .or_else(|| value.get("message"))
                    .or_else(|| value.get("content")),
            )
            .unwrap_or_else(|| line.to_string());
            vec![LogEntry::ResultMessage(message)]
        }
        _ => Vec::new(),
    }
}

fn parse_assistant_message_entries(value: &Value) -> Vec<LogEntry> {
    let mut out = Vec::new();

    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for block in content {
            let block_type = block
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();

            match block_type.as_str() {
                "text" => {
                    if let Some(text) =
                        extract_textish(Some(block)).and_then(|t| normalize_multiline_text(&t))
                    {
                        out.push(LogEntry::AssistantText(text));
                    }
                }
                "thinking" => {
                    if let Some(text) =
                        extract_thinking_text(block).and_then(|t| normalize_multiline_text(&t))
                    {
                        out.push(LogEntry::Thinking(text));
                    }
                }
                "tool_use" => {
                    let name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("unknown")
                        .to_string();
                    let summary = extract_tool_summary(&name, block.get("input"));
                    out.push(LogEntry::ToolUse { name, summary });
                }
                _ => {
                    if let Some(text) =
                        extract_textish(Some(block)).and_then(|t| normalize_multiline_text(&t))
                    {
                        out.push(LogEntry::AssistantText(text));
                    }
                }
            }
        }
    }

    if out.is_empty()
        && let Some(text) = extract_textish(
            value
                .get("text")
                .or_else(|| value.pointer("/delta/text"))
                .or_else(|| value.pointer("/message/content"))
                .or_else(|| value.get("content"))
                .or_else(|| value.get("message")),
        )
        .and_then(|t| normalize_multiline_text(&t))
    {
        out.push(LogEntry::AssistantText(text));
    }

    out
}

fn parse_user_message_entries(value: &Value) -> Vec<LogEntry> {
    let mut out = Vec::new();
    let mut has_tool_result = false;
    let is_synthetic = value
        .get("isSynthetic")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    if let Some(content) = value.pointer("/message/content").and_then(Value::as_array) {
        for block in content {
            let block_type = block
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_ascii_lowercase();

            match block_type.as_str() {
                "tool_result" => {
                    if let Some(text) = extract_textish(block.get("content"))
                        .or_else(|| extract_textish(Some(block)))
                        .and_then(|t| normalize_multiline_text(&t))
                    {
                        has_tool_result = true;
                        out.push(LogEntry::ToolResult {
                            content: text,
                            tool_use_id: block
                                .get("tool_use_id")
                                .and_then(Value::as_str)
                                .map(ToOwned::to_owned),
                        });
                    }
                }
                "text" => {
                    if let Some(text) =
                        extract_textish(Some(block)).and_then(|t| normalize_multiline_text(&t))
                    {
                        if is_synthetic {
                            out.push(LogEntry::Raw(summarize_synthetic_user_text(&text)));
                        } else {
                            out.push(LogEntry::SystemMessage(format!("[user] {text}")));
                        }
                    }
                }
                _ => {}
            }
        }
    }

    if !has_tool_result
        && let Some(text) = extract_tool_use_result_text(value).and_then(|t| {
            // Keep stdout/stderr readable, but drop accidental leading/trailing empty lines.
            normalize_multiline_text(&t)
        })
    {
        out.push(LogEntry::ToolResult {
            content: text,
            tool_use_id: extract_user_tool_use_id(value),
        });
    }

    out
}

fn extract_user_tool_use_id(value: &Value) -> Option<String> {
    value
        .pointer("/message/content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find_map(|item| {
                item.get("tool_use_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
        })
}

fn extract_tool_use_result_text(value: &Value) -> Option<String> {
    let tool_use_result = value.get("tool_use_result")?;
    match tool_use_result {
        Value::String(text) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        Value::Object(map) => {
            let stdout = map
                .get("stdout")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let stderr = map
                .get("stderr")
                .and_then(Value::as_str)
                .unwrap_or_default();

            let mut merged = String::new();
            if !stdout.trim().is_empty() {
                merged.push_str(stdout.trim());
            }
            if !stderr.trim().is_empty() {
                if !merged.is_empty() {
                    merged.push('\n');
                }
                merged.push_str(stderr.trim());
            }

            if !merged.is_empty() {
                Some(merged)
            } else {
                extract_textish(Some(tool_use_result))
            }
        }
        _ => None,
    }
}

fn extract_thinking_text(value: &Value) -> Option<String> {
    value
        .get("thinking")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| extract_textish(Some(value)))
}

fn parse_jsonl_fallback_entry(line: &str) -> Option<LogEntry> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    let object = value.as_object()?;

    let event_type = object.get("type").and_then(Value::as_str).unwrap_or("json");
    let subtype = object
        .get("subtype")
        .and_then(Value::as_str)
        .unwrap_or_default();

    if event_type.eq_ignore_ascii_case("user") {
        return Some(user_fallback_entry(&value, subtype));
    }

    let mut summary = if subtype.is_empty() {
        format!("[json] type={event_type}")
    } else {
        format!("[json] type={event_type} subtype={subtype}")
    };

    let detail = extract_textish(
        object
            .get("message")
            .or_else(|| object.get("result"))
            .or_else(|| object.get("error"))
            .or_else(|| object.get("status"))
            .or_else(|| object.get("detail")),
    )
    .map(|text| truncate_text(&sanitize_line_for_display(&text), 180));

    if let Some(text) = detail
        && !text.is_empty()
    {
        summary.push_str(" | ");
        summary.push_str(&text);
    }

    Some(LogEntry::SystemMessage(summary))
}

fn user_fallback_entry(value: &Value, subtype: &str) -> LogEntry {
    let is_synthetic = value
        .get("isSynthetic")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if is_synthetic
        && let Some(text) = value
            .pointer("/message/content")
            .and_then(|v| extract_textish(Some(v)))
            .and_then(|v| normalize_multiline_text(&v))
    {
        return LogEntry::Raw(summarize_synthetic_user_text(&text));
    }

    let detail = extract_textish(
        value
            .get("message")
            .or_else(|| value.get("content"))
            .or_else(|| value.get("result"))
            .or_else(|| value.get("error"))
            .or_else(|| value.get("detail"))
            .or_else(|| value.pointer("/tool_use_result/stdout"))
            .or_else(|| value.pointer("/tool_use_result/stderr")),
    )
    .and_then(|text| normalize_multiline_text(&text))
    .map(|text| truncate_text(&sanitize_line_for_display(&text), 180));

    let summary = if let Some(text) = detail {
        if subtype.is_empty() {
            format!("[user] {text}")
        } else {
            format!("[user:{subtype}] {text}")
        }
    } else if subtype.is_empty() {
        "[user] structured message".to_string()
    } else {
        format!("[user] subtype={subtype}")
    };

    LogEntry::Raw(summary)
}

fn summarize_synthetic_user_text(text: &str) -> String {
    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or("");

    if let Some(path) = first_line.strip_prefix("Base directory for this skill: ") {
        let skill = path
            .trim()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("skill");
        return format!("[skill] {skill} instructions injected");
    }

    if first_line.is_empty() {
        "[skill] synthetic context injected".to_string()
    } else {
        format!("[skill] {}", truncate_text(first_line, 120))
    }
}

fn format_status(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "pending",
        TaskStatus::Running => "running",
        TaskStatus::Completed => "completed",
        TaskStatus::Failed => "failed",
        TaskStatus::Cancelled => "cancelled",
    }
}

fn sanitize_line_for_display(input: &str) -> String {
    let no_carriage = input.rsplit('\r').next().unwrap_or(input);
    let no_ansi = strip_ansi_escape(no_carriage);
    no_ansi.trim().to_string()
}

fn normalize_multiline_text(input: &str) -> Option<String> {
    let stripped = strip_ansi_escape(input).replace('\r', "");
    if stripped.trim().is_empty() {
        return None;
    }

    let mut lines: Vec<String> = stripped
        .lines()
        .map(|line| line.trim_end().to_string())
        .collect();
    while matches!(lines.first(), Some(line) if line.trim().is_empty()) {
        lines.remove(0);
    }
    while matches!(lines.last(), Some(line) if line.trim().is_empty()) {
        let _ = lines.pop();
    }

    let mut compact = Vec::with_capacity(lines.len());
    let mut previous_blank = false;
    for line in lines {
        let is_blank = line.trim().is_empty();
        if is_blank && previous_blank {
            continue;
        }
        previous_blank = is_blank;
        compact.push(line);
    }

    if compact.is_empty() {
        None
    } else {
        Some(compact.join("\n"))
    }
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }

    let mut out = String::with_capacity(max_chars + 1);
    for (idx, ch) in text.chars().enumerate() {
        if idx >= max_chars {
            break;
        }
        out.push(ch);
    }
    out.push('…');
    out
}

fn strip_ansi_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            if chars.peek().copied() == Some('[') {
                let _ = chars.next();
                for next in chars.by_ref() {
                    let code = next as u32;
                    if (0x40..=0x7E).contains(&code) {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(ch);
    }

    out
}

fn extract_textish(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(s) => {
            if s.is_empty() {
                None
            } else {
                Some(s.to_string())
            }
        }
        Value::Array(items) => {
            let parts: Vec<String> = items
                .iter()
                .filter_map(|item| extract_textish(Some(item)))
                .filter(|part| !part.is_empty())
                .collect();
            if parts.is_empty() {
                None
            } else {
                Some(parts.join("\n"))
            }
        }
        Value::Object(map) => {
            if let Some(text) = map.get("text").and_then(Value::as_str) {
                return Some(text.to_string());
            }
            for key in [
                "message", "content", "result", "output", "delta", "thinking",
            ] {
                if let Some(found) = extract_textish(map.get(key)) {
                    return Some(found);
                }
            }
            None
        }
        Value::Number(number) => Some(number.to_string()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Null => None,
    }
}

fn format_value(value: &Value) -> String {
    match value {
        Value::String(s) => s.to_string(),
        Value::Null => "null".to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Array(_) | Value::Object(_) => {
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_assistant_tool_use_block_from_stream_json() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Bash","input":{"command":"echo hello"}}]}}"#;
        let entries = parse_claude_log_entries(line);

        assert_eq!(entries.len(), 1);
        match &entries[0] {
            LogEntry::ToolUse { name, summary } => {
                assert_eq!(name, "Bash");
                assert_eq!(summary, "echo hello");
            }
            other => panic!("unexpected entry: {other:?}"),
        };
    }

    #[test]
    fn parse_user_tool_result_block_from_stream_json() {
        let line =
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"done"}]}}"#;
        let entries = parse_claude_log_entries(line);

        assert_eq!(entries.len(), 1);
        match &entries[0] {
            LogEntry::ToolResult {
                content,
                tool_use_id,
            } => {
                assert_eq!(content, "done");
                assert!(tool_use_id.is_none());
            }
            other => panic!("unexpected entry: {other:?}"),
        };
    }

    #[test]
    fn parse_assistant_thinking_block_from_stream_json() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"plan first"}]}}"#;
        let entries = parse_claude_log_entries(line);

        assert_eq!(entries.len(), 1);
        match &entries[0] {
            LogEntry::Thinking(content) => assert_eq!(content, "plan first"),
            other => panic!("unexpected entry: {other:?}"),
        };
    }

    #[test]
    fn user_json_fallback_is_summarized() {
        let mut app = App::new("task-1".to_string());
        app.handle_raw_log(r#"{"type":"user","subtype":"noop"}"#.to_string());
        assert_eq!(app.kernel_logs.len(), 1);
        match &app.kernel_logs[0] {
            LogEntry::Raw(line) => assert_eq!(line, "[user] subtype=noop"),
            other => panic!("unexpected entry: {other:?}"),
        }
    }

    #[test]
    fn synthetic_user_text_is_summarized() {
        let line = r#"{"type":"user","isSynthetic":true,"message":{"role":"user","content":[{"type":"text","text":"Base directory for this skill: /home/mindbox/.claude/skills/gpu-discovery-skill\n\n# GPU Discovery"}]}}"#;
        let entries = parse_claude_log_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            LogEntry::Raw(line) => {
                assert_eq!(line, "[skill] gpu-discovery-skill instructions injected")
            }
            other => panic!("unexpected entry: {other:?}"),
        }
    }

    #[test]
    fn user_tool_result_string_is_supported() {
        let line = r#"{"type":"user","message":{"role":"user","content":[]},"tool_use_result":"Error: Exit code 2"}"#;
        let entries = parse_claude_log_entries(line);
        assert_eq!(entries.len(), 1);
        match &entries[0] {
            LogEntry::ToolResult { content, .. } => assert_eq!(content, "Error: Exit code 2"),
            other => panic!("unexpected entry: {other:?}"),
        }
    }

    #[test]
    fn consecutive_tool_result_with_same_tool_use_id_updates_in_place() {
        let mut app = App::new("task-1".to_string());
        app.handle_raw_log(
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-1","content":"line 1"}]}}"#
                .to_string(),
        );
        app.handle_raw_log(
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"tool-1","content":"line 1\nline 2"}]}}"#
                .to_string(),
        );

        assert_eq!(app.kernel_logs.len(), 1);
        match &app.kernel_logs[0] {
            LogEntry::ToolResult {
                content,
                tool_use_id,
            } => {
                assert_eq!(content, "line 1\nline 2");
                assert_eq!(tool_use_id.as_deref(), Some("tool-1"));
            }
            other => panic!("unexpected entry: {other:?}"),
        }
    }
}
