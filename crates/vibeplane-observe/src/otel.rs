//! OpenTelemetry — the cost, token and latency channel.
//!
//! Claude Code exports OTLP from every entrypoint: the CLI, the VS Code
//! extension, the desktop app and the SDK. It is the only documented channel
//! with per-request cost, and `app.entrypoint` on every record is what tells
//! the board which surface a session belongs to.
//!
//! The receiver speaks OTLP/HTTP with the JSON encoding, so there is no
//! collector to install and no protobuf dependency. The parser is deliberately
//! tolerant: unknown records are skipped, and a field that changes shape costs
//! one event rather than the batch.
//!
//! Prompt and response text never appear here. Claude Code redacts them unless
//! `OTEL_LOG_USER_PROMPTS` is set, and Vibeplane does not set it.

use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use vibeplane_domain::event::{ApiUsage, Event};

/// One parsed telemetry record: an event and the session it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct OtelRecord {
    pub session_id: String,
    pub entrypoint: Option<String>,
    /// The repository the session is in, from `vcs.repository.url.full`
    /// (Claude Code v2.1.269+). Correlates a session to a project without a
    /// filesystem lookup.
    pub repo_url: Option<String>,
    pub event: Event,
}

#[derive(Debug, Deserialize)]
pub struct LogsPayload {
    #[serde(default, rename = "resourceLogs")]
    resource_logs: Vec<ResourceLogs>,
}

#[derive(Debug, Deserialize)]
struct ResourceLogs {
    #[serde(default)]
    resource: Option<Resource>,
    #[serde(default, rename = "scopeLogs")]
    scope_logs: Vec<ScopeLogs>,
}

#[derive(Debug, Deserialize)]
struct Resource {
    #[serde(default)]
    attributes: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
struct ScopeLogs {
    #[serde(default, rename = "logRecords")]
    log_records: Vec<LogRecord>,
}

#[derive(Debug, Deserialize)]
struct LogRecord {
    #[serde(default)]
    body: Option<AnyValue>,
    #[serde(default)]
    attributes: Vec<KeyValue>,
}

#[derive(Debug, Deserialize)]
struct KeyValue {
    key: String,
    #[serde(default)]
    value: Option<AnyValue>,
}

/// OTLP's tagged value union. The JSON encoding sends 64-bit integers as
/// strings, so every accessor tolerates both.
#[derive(Debug, Deserialize)]
struct AnyValue {
    #[serde(default, rename = "stringValue")]
    string_value: Option<String>,
    #[serde(default, rename = "intValue")]
    int_value: Option<Value>,
    #[serde(default, rename = "doubleValue")]
    double_value: Option<f64>,
    #[serde(default, rename = "boolValue")]
    bool_value: Option<bool>,
}

impl AnyValue {
    fn as_string(&self) -> Option<String> {
        if let Some(s) = &self.string_value {
            return Some(s.clone());
        }
        if let Some(v) = &self.int_value {
            return Some(v.to_string().trim_matches('"').to_string());
        }
        self.double_value
            .map(|d| d.to_string())
            .or_else(|| self.bool_value.map(|b| b.to_string()))
    }

    fn as_f64(&self) -> Option<f64> {
        if let Some(d) = self.double_value {
            return Some(d);
        }
        match &self.int_value {
            Some(Value::Number(n)) => n.as_f64(),
            Some(Value::String(s)) => s.parse().ok(),
            _ => self.string_value.as_ref().and_then(|s| s.parse().ok()),
        }
    }

    fn as_u64(&self) -> Option<u64> {
        self.as_f64().map(|f| f.max(0.0) as u64)
    }
}

type Attrs = BTreeMap<String, AnyValue>;

fn collect(kvs: Vec<KeyValue>) -> Attrs {
    kvs.into_iter()
        .filter_map(|kv| kv.value.map(|v| (kv.key, v)))
        .collect()
}

fn s(a: &Attrs, k: &str) -> Option<String> {
    a.get(k).and_then(|v| v.as_string())
}
fn f(a: &Attrs, k: &str) -> f64 {
    a.get(k).and_then(|v| v.as_f64()).unwrap_or(0.0)
}
fn u(a: &Attrs, k: &str) -> u64 {
    a.get(k).and_then(|v| v.as_u64()).unwrap_or(0)
}

/// Parses an OTLP/HTTP logs payload into records.
pub fn parse_logs(body: &[u8]) -> Result<Vec<OtelRecord>, serde_json::Error> {
    let payload: LogsPayload = serde_json::from_slice(body)?;
    let mut out = Vec::new();

    for rl in payload.resource_logs {
        let resource = rl
            .resource
            .map(|r| collect(r.attributes))
            .unwrap_or_default();
        for sl in rl.scope_logs {
            for rec in sl.log_records {
                let name = rec
                    .body
                    .as_ref()
                    .and_then(|b| b.as_string())
                    .unwrap_or_default();
                let attrs = collect(rec.attributes);
                // The event name lives in the body on some versions and in an
                // `event.name` attribute on others. Reading both costs nothing
                // and survives the change either way.
                let name = s(&attrs, "event.name").unwrap_or(name);

                let session_id =
                    match s(&attrs, "session.id").or_else(|| s(&resource, "session.id")) {
                        Some(id) => id,
                        // A record with no session cannot be attributed to a run,
                        // and guessing would put another session's cost on the
                        // wrong row.
                        None => continue,
                    };
                let entrypoint =
                    s(&attrs, "app.entrypoint").or_else(|| s(&resource, "app.entrypoint"));
                let repo_url = s(&attrs, "vcs.repository.url.full")
                    .or_else(|| s(&resource, "vcs.repository.url.full"));

                let Some(event) = log_to_event(&name, &attrs) else {
                    continue;
                };
                out.push(OtelRecord {
                    session_id,
                    entrypoint,
                    repo_url,
                    event,
                });
            }
        }
    }
    Ok(out)
}

fn log_to_event(name: &str, a: &Attrs) -> Option<Event> {
    match name {
        "claude_code.api_request" => Some(Event::ApiRequest {
            usage: ApiUsage {
                model: s(a, "model"),
                cost_usd: f(a, "cost_usd"),
                input_tokens: u(a, "input_tokens"),
                output_tokens: u(a, "output_tokens"),
                cache_read_tokens: u(a, "cache_read_tokens"),
                cache_creation_tokens: u(a, "cache_creation_tokens"),
            },
        }),
        "claude_code.api_error" => Some(Event::ApiError {
            error: s(a, "error").unwrap_or_else(|| "api error".into()),
            status: a
                .get("status_code")
                .and_then(|v| v.as_f64())
                .map(|f| f as i64),
        }),
        "claude_code.tool_result" => Some(Event::ToolFinished {
            tool: s(a, "tool_name").unwrap_or_default(),
            ok: s(a, "success").map(|v| v == "true").unwrap_or(true),
            duration_ms: a.get("duration_ms").and_then(|v| v.as_u64()),
        }),
        "claude_code.tool_decision" => Some(Event::PermissionDecided {
            tool: s(a, "tool_name").unwrap_or_default(),
            decision: s(a, "decision").unwrap_or_default(),
            by: s(a, "source").unwrap_or_else(|| "unknown".into()),
        }),
        // Prompts and responses carry only their length here, which the hook
        // channel already reports more promptly. Recording them twice would
        // double every count on the board.
        _ => None,
    }
}

/// Parses an OTLP/HTTP metrics payload. Metrics duplicate what the log records
/// already say per request, so only the session ids are extracted — enough to
/// prove the channel is alive in diagnostics.
pub fn parse_metrics_sessions(body: &[u8]) -> Vec<String> {
    let Ok(v) = serde_json::from_slice::<Value>(body) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    collect_session_ids(&v, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_session_ids(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::Object(o) => {
            if o.get("key").and_then(|k| k.as_str()) == Some("session.id")
                && let Some(id) = o
                    .get("value")
                    .and_then(|x| x.get("stringValue"))
                    .and_then(|x| x.as_str())
            {
                out.push(id.to_string());
            }
            for x in o.values() {
                collect_session_ids(x, out);
            }
        }
        Value::Array(a) => a.iter().for_each(|x| collect_session_ids(x, out)),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn attr(k: &str, v: serde_json::Value) -> serde_json::Value {
        json!({"key": k, "value": v})
    }

    #[test]
    fn an_api_request_carries_cost_and_tokens() {
        let body = json!({"resourceLogs": [{
            "resource": {"attributes": [attr("service.name", json!({"stringValue": "claude-code"}))]},
            "scopeLogs": [{"logRecords": [{
                "body": {"stringValue": "claude_code.api_request"},
                "attributes": [
                    attr("session.id", json!({"stringValue": "s1"})),
                    attr("app.entrypoint", json!({"stringValue": "claude-vscode"})),
                    attr("model", json!({"stringValue": "claude-opus-5"})),
                    attr("cost_usd", json!({"doubleValue": 0.0342})),
                    // The JSON encoding sends 64-bit ints as strings.
                    attr("input_tokens", json!({"intValue": "18000"})),
                    attr("output_tokens", json!({"intValue": "512"})),
                    attr("cache_read_tokens", json!({"intValue": "120000"}))
                ]
            }]}]
        }]});
        let recs = parse_logs(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].session_id, "s1");
        assert_eq!(recs[0].entrypoint.as_deref(), Some("claude-vscode"));
        match &recs[0].event {
            Event::ApiRequest { usage } => {
                assert!((usage.cost_usd - 0.0342).abs() < 1e-9);
                assert_eq!(usage.input_tokens, 18_000);
                assert_eq!(usage.cache_read_tokens, 120_000);
                assert_eq!(usage.context_tokens(), 138_000);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_event_name_may_come_from_an_attribute() {
        let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
            "attributes": [
                attr("event.name", json!({"stringValue": "claude_code.api_error"})),
                attr("session.id", json!({"stringValue": "s1"})),
                attr("error", json!({"stringValue": "overloaded"})),
                attr("status_code", json!({"intValue": "529"}))
            ]
        }]}]}]});
        let recs = parse_logs(&serde_json::to_vec(&body).unwrap()).unwrap();
        match &recs[0].event {
            Event::ApiError { error, status } => {
                assert_eq!(error, "overloaded");
                assert_eq!(*status, Some(529));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_record_without_a_session_is_dropped() {
        // Attributing cost to the wrong run is worse than losing the record.
        let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
            "body": {"stringValue": "claude_code.api_request"},
            "attributes": [attr("cost_usd", json!({"doubleValue": 1.0}))]
        }]}]}]});
        assert!(
            parse_logs(&serde_json::to_vec(&body).unwrap())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn the_repository_attribute_is_picked_up() {
        let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
            "body": {"stringValue": "claude_code.api_request"},
            "attributes": [
                attr("session.id", json!({"stringValue": "s1"})),
                attr("vcs.repository.url.full", json!({"stringValue": "https://github.com/hupe1980/vibeplane"}))
            ]
        }]}]}]});
        let recs = parse_logs(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(
            recs[0].repo_url.as_deref(),
            Some("https://github.com/hupe1980/vibeplane")
        );
    }

    #[test]
    fn prompt_records_are_not_turned_into_events() {
        // The hook channel already reports prompts, and sooner. Counting them
        // twice would double every figure on the board.
        let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
            "body": {"stringValue": "claude_code.user_prompt"},
            "attributes": [attr("session.id", json!({"stringValue": "s1"}))]
        }]}]}]});
        assert!(
            parse_logs(&serde_json::to_vec(&body).unwrap())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn garbage_is_an_error_not_a_panic() {
        assert!(parse_logs(b"not json").is_err());
        assert!(parse_metrics_sessions(b"not json").is_empty());
    }

    #[test]
    fn metrics_yield_their_session_ids() {
        let body = json!({"resourceMetrics": [{"scopeMetrics": [{"metrics": [{
            "name": "claude_code.token.usage",
            "sum": {"dataPoints": [{
                "asInt": "100",
                "attributes": [attr("session.id", json!({"stringValue": "s9"}))]
            }]}
        }]}]}]});
        assert_eq!(
            parse_metrics_sessions(&serde_json::to_vec(&body).unwrap()),
            vec!["s9".to_string()]
        );
    }
}
