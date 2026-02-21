use chrono::{DateTime, TimeZone, Utc};
use serde_json::Value;

use crate::{TaskEvent, TaskStatus};

pub fn format_stream_event(json_line: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(json_line) else {
        return json_line.to_string();
    };

    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    match event_type.as_str() {
        "assistant" => extract_assistant_text(&value).unwrap_or_else(|| json_line.to_string()),
        "tool_use" => format_tool_use(&value).unwrap_or_else(|| json_line.to_string()),
        "tool_result" => format_tool_result(&value).unwrap_or_else(|| json_line.to_string()),
        "system" => {
            let message = extract_textish(
                value
                    .get("message")
                    .or_else(|| value.get("text"))
                    .or_else(|| value.get("content")),
            )
            .unwrap_or_else(|| json_line.to_string());
            format!("[system] {message}")
        }
        "result" => {
            let text = extract_textish(
                value
                    .get("text")
                    .or_else(|| value.get("result"))
                    .or_else(|| value.get("message"))
                    .or_else(|| value.get("content")),
            )
            .unwrap_or_else(|| json_line.to_string());
            format!("=== Done ===\n{text}")
        }
        _ => json_line.to_string(),
    }
}

pub fn format_task_event(event: &TaskEvent) -> String {
    match event {
        TaskEvent::StatusUpdate {
            status, message, ..
        } => {
            format!("[status: {}] {message}", format_status(*status))
        }
        TaskEvent::Metric { metric, .. } => {
            let step = metric
                .step
                .map(|value| value.to_string())
                .unwrap_or_else(|| "none".to_string());
            format!("[metric] {}={} step={step}", metric.name, metric.value)
        }
        TaskEvent::Error { message, .. } => format!("[error] {message}"),
        TaskEvent::Log { message, .. } => format_stream_event(message),
    }
}

pub fn format_log_line(line: &str) -> String {
    if let Ok(event) = serde_json::from_str::<TaskEvent>(line) {
        return format_task_event(&event);
    }
    format_stream_event(line)
}

pub fn parse_log_timestamp(line: &str) -> Option<DateTime<Utc>> {
    if let Ok(event) = serde_json::from_str::<TaskEvent>(line) {
        return Some(task_event_timestamp(&event));
    }

    let value = serde_json::from_str::<Value>(line).ok()?;
    for key in ["timestamp", "created_at", "time"] {
        if let Some(parsed) = value.get(key).and_then(parse_timestamp_value) {
            return Some(parsed);
        }
    }
    None
}

pub fn task_event_timestamp(event: &TaskEvent) -> DateTime<Utc> {
    match event {
        TaskEvent::StatusUpdate { timestamp, .. }
        | TaskEvent::Log { timestamp, .. }
        | TaskEvent::Metric { timestamp, .. }
        | TaskEvent::Error { timestamp, .. } => *timestamp,
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

fn format_tool_use(value: &Value) -> Option<String> {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .or_else(|| value.get("tool_name").and_then(Value::as_str))
        .or_else(|| value.pointer("/tool/name").and_then(Value::as_str))
        .unwrap_or("unknown");
    let input = value
        .get("input")
        .or_else(|| value.get("tool_input"))
        .or_else(|| value.get("arguments"))
        .map(format_value);
    match input {
        Some(input) if !input.is_empty() => Some(format!("> Tool: {name}\n{input}")),
        _ => Some(format!("> Tool: {name}")),
    }
}

fn format_tool_result(value: &Value) -> Option<String> {
    let content = value
        .get("content")
        .or_else(|| value.get("result"))
        .or_else(|| value.get("message"))
        .map(format_value)
        .or_else(|| extract_textish(Some(value)))?;
    Some(format!("> Result:\n{content}"))
}

fn extract_assistant_text(value: &Value) -> Option<String> {
    extract_textish(
        value
            .get("text")
            .or_else(|| value.pointer("/delta/text"))
            .or_else(|| value.pointer("/message/content"))
            .or_else(|| value.get("content"))
            .or_else(|| value.get("message")),
    )
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
            for key in ["message", "content", "result", "output", "delta"] {
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

fn parse_timestamp_value(value: &Value) -> Option<DateTime<Utc>> {
    if let Some(text) = value.as_str() {
        let parsed = DateTime::parse_from_rfc3339(text).ok()?;
        return Some(parsed.with_timezone(&Utc));
    }

    let millis = if let Some(num) = value.as_i64() {
        Some(num)
    } else if let Some(num) = value.as_u64() {
        i64::try_from(num).ok()
    } else {
        None
    }?;

    if millis > 10_000_000_000 {
        let secs = millis / 1_000;
        let nanos = ((millis % 1_000) * 1_000_000) as u32;
        Utc.timestamp_opt(secs, nanos).single()
    } else {
        Utc.timestamp_opt(millis, 0).single()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_assistant_event_text() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#;
        assert_eq!(format_stream_event(line), "hello");
    }

    #[test]
    fn formats_tool_use_event() {
        let line = r#"{"type":"tool_use","name":"shell","input":{"cmd":"ls -la"}}"#;
        let out = format_stream_event(line);
        assert!(out.starts_with("> Tool: shell\n"));
        assert!(out.contains("\"cmd\": \"ls -la\""));
    }

    #[test]
    fn formats_task_status_event_line() {
        let line = r#"{"type":"status_update","status":"completed","message":"task completed","timestamp":"2026-02-21T00:00:00Z"}"#;
        assert_eq!(format_log_line(line), "[status: completed] task completed");
    }
}
