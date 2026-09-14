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
//! **Two dialects arrive on this one transport**, which is the shape the whole
//! observe layer is growing into. Claude Code exports *log records* named
//! `claude_code.*`; GitHub Copilot exports *traces* whose *"signal names and
//! attributes follow the OTel GenAI Semantic Conventions"*, as does Codex. So
//! the receiver reads both and maps them onto the same event model — the
//! conventions are an ingest dialect here and never the internal type, because
//! they are explicitly pre-stable and the reducer the whole board is a pure
//! function of is the wrong place to inherit somebody else's version churn.
//!
//! One asymmetry is worth knowing before reading a board: **`cost_usd` is
//! Claude Code's own extension.** The GenAI conventions carry tokens, models,
//! tool names and durations and have no notion of money, so a Copilot run shows
//! tokens and no dollars, and a spending ceiling cannot bind it.
//!
//! Prompt and response text never appear here. Claude Code redacts them unless
//! `OTEL_LOG_USER_PROMPTS` is set, and Copilot unless
//! `OTEL_INSTRUMENTATION_GENAI_CAPTURE_MESSAGE_CONTENT` is set. Vibeplane sets
//! neither.

use crate::core::event::{ApiUsage, Event};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;

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
                // Telemetry reports one request, not a window level; the level
                // is derived from the three token counts above.
                context_level: None,
            },
        }),
        "claude_code.api_error" => Some(Event::ApiError {
            error: s(a, "error").unwrap_or_else(|| "api error".into()),
            status: a
                .get("status_code")
                .and_then(|v| v.as_f64())
                .map(|f| f as i64),
        }),
        "claude_code.tool_decision" => Some(Event::PermissionDecided {
            tool: s(a, "tool_name").unwrap_or_default(),
            decision: s(a, "decision").unwrap_or_default(),
            by: s(a, "source").unwrap_or_else(|| "unknown".into()),
        }),
        // Everything else here is something the hook channel already reports,
        // and reports sooner. Prompts and responses carry only their length.
        // `tool_result` is the one that was being taken as well as
        // `PostToolUse`/`PostToolUseFailure` — so a failing tool call counted
        // as two errors on the board, telemetry flushing seconds after the hook
        // that had already said it. One channel per fact.
        _ => None,
    }
}

#[derive(Debug, Deserialize)]
pub struct TracesPayload {
    #[serde(default, rename = "resourceSpans")]
    resource_spans: Vec<ResourceSpans>,
}

#[derive(Debug, Deserialize)]
struct ResourceSpans {
    #[serde(default)]
    resource: Option<Resource>,
    #[serde(default, rename = "scopeSpans")]
    scope_spans: Vec<ScopeSpans>,
}

#[derive(Debug, Deserialize)]
struct ScopeSpans {
    #[serde(default)]
    spans: Vec<Span>,
}

#[derive(Debug, Deserialize)]
struct Span {
    #[serde(default)]
    name: String,
    #[serde(default)]
    attributes: Vec<KeyValue>,
    #[serde(default, rename = "startTimeUnixNano")]
    start: Option<Value>,
    #[serde(default, rename = "endTimeUnixNano")]
    end: Option<Value>,
    #[serde(default)]
    status: Option<SpanStatus>,
}

#[derive(Debug, Deserialize)]
struct SpanStatus {
    #[serde(default)]
    code: Option<i64>,
}

fn nanos(v: &Option<Value>) -> Option<u64> {
    match v {
        Some(Value::String(s)) => s.parse().ok(),
        Some(Value::Number(n)) => n.as_u64(),
        _ => None,
    }
}

/// Parses an OTLP/HTTP traces payload written to the GenAI semantic
/// conventions.
///
/// The conventions nest a run as one `invoke_agent` span with `chat` and
/// `execute_tool` spans beneath it. Only the leaves say anything the board does
/// not already know, so the parent is read for identity and skipped otherwise.
///
/// Tolerant in the same way `parse_logs` is: a span that names no session, or
/// no operation this maps, costs one span rather than the batch. That is the
/// rule that lets a pre-stable convention change under us without taking the
/// channel down.
pub fn parse_traces(body: &[u8]) -> Result<Vec<OtelRecord>, serde_json::Error> {
    let payload: TracesPayload = serde_json::from_slice(body)?;
    let mut out = Vec::new();

    for rs in payload.resource_spans {
        let resource = rs
            .resource
            .map(|r| collect(r.attributes))
            .unwrap_or_default();
        for ss in rs.scope_spans {
            for span in ss.spans {
                let attrs = collect(span.attributes);
                // `gen_ai.conversation.id` is the conventions' name for what
                // this product calls a session. `session.id` is read first
                // anyway, because an exporter that already sets it is telling
                // us the answer in the vocabulary the rest of this file uses.
                let Some(session_id) = s(&attrs, "session.id")
                    .or_else(|| s(&attrs, "gen_ai.conversation.id"))
                    .or_else(|| s(&resource, "session.id"))
                    .or_else(|| s(&resource, "gen_ai.conversation.id"))
                else {
                    continue;
                };
                // The vendor, from the service name the exporter sets —
                // `github-copilot` by default. It is the entrypoint's job here:
                // the board has to be able to say which agent a row belongs to,
                // and on this dialect nothing else says.
                let entrypoint = s(&attrs, "app.entrypoint")
                    .or_else(|| s(&resource, "app.entrypoint"))
                    .or_else(|| s(&resource, "service.name"));
                let repo_url = s(&attrs, "vcs.repository.url.full")
                    .or_else(|| s(&resource, "vcs.repository.url.full"));

                // The operation, from the attribute if it is there and from the
                // span name otherwise — the conventions name a span
                // `chat <model>` or `execute_tool <tool>`, so the first word is
                // the operation whenever the attribute is missing.
                let op = s(&attrs, "gen_ai.operation.name")
                    .or_else(|| span.name.split_whitespace().next().map(str::to_string))
                    .unwrap_or_default();

                let duration_ms = match (nanos(&span.start), nanos(&span.end)) {
                    (Some(a), Some(b)) if b >= a => Some((b - a) / 1_000_000),
                    _ => None,
                };

                let event = match op.as_str() {
                    "chat" | "text_completion" | "generate_content" => Event::ApiRequest {
                        usage: ApiUsage {
                            model: s(&attrs, "gen_ai.response.model")
                                .or_else(|| s(&attrs, "gen_ai.request.model")),
                            // Not a field these conventions have. Reporting a
                            // zero is honest — it is what was observed — and
                            // `work show` already says when a bound cannot see
                            // what something cost.
                            cost_usd: 0.0,
                            input_tokens: u(&attrs, "gen_ai.usage.input_tokens"),
                            output_tokens: u(&attrs, "gen_ai.usage.output_tokens"),
                            cache_read_tokens: u(&attrs, "gen_ai.usage.cache_read_input_tokens"),
                            cache_creation_tokens: 0,
                            context_level: None,
                        },
                    },
                    "execute_tool" => Event::ToolFinished {
                        tool: s(&attrs, "gen_ai.tool.name")
                            .or_else(|| span.name.split_whitespace().nth(1).map(str::to_string))
                            .unwrap_or_else(|| "tool".into()),
                        // Status code 2 is `STATUS_CODE_ERROR`; anything else,
                        // including an unset status, is not a failure.
                        ok: span.status.as_ref().and_then(|s| s.code) != Some(2),
                        duration_ms,
                    },
                    // `invoke_agent` is the parent of everything above and adds
                    // nothing the children do not carry; the rest is a shape
                    // this map does not know, which costs one span.
                    _ => continue,
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
    #[test]
    fn a_genai_trace_becomes_the_same_events_a_claude_log_does() {
        // The second dialect: GitHub Copilot and Codex export traces on the
        // GenAI semantic conventions where Claude Code exports `claude_code.*`
        // log records. One event model on the far side of both.
        let body = serde_json::json!({
            "resourceSpans": [{
                "resource": {"attributes": [
                    {"key": "service.name", "value": {"stringValue": "github-copilot"}}
                ]},
                "scopeSpans": [{"spans": [
                    {
                        "name": "invoke_agent copilot",
                        "attributes": [
                            {"key": "gen_ai.conversation.id", "value": {"stringValue": "s-1"}},
                            {"key": "gen_ai.operation.name", "value": {"stringValue": "invoke_agent"}}
                        ]
                    },
                    {
                        "name": "chat gpt-5",
                        "attributes": [
                            {"key": "gen_ai.conversation.id", "value": {"stringValue": "s-1"}},
                            {"key": "gen_ai.operation.name", "value": {"stringValue": "chat"}},
                            {"key": "gen_ai.response.model", "value": {"stringValue": "gpt-5"}},
                            {"key": "gen_ai.usage.input_tokens", "value": {"intValue": "1200"}},
                            {"key": "gen_ai.usage.output_tokens", "value": {"intValue": 340}}
                        ]
                    },
                    {
                        "name": "execute_tool shell",
                        "startTimeUnixNano": "1000000000",
                        "endTimeUnixNano": "1002500000",
                        "status": {"code": 2},
                        "attributes": [
                            {"key": "gen_ai.conversation.id", "value": {"stringValue": "s-1"}},
                            {"key": "gen_ai.operation.name", "value": {"stringValue": "execute_tool"}},
                            {"key": "gen_ai.tool.name", "value": {"stringValue": "shell"}}
                        ]
                    }
                ]}]
            }]
        });
        let recs = parse_traces(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(recs.len(), 2, "the parent span carries nothing of its own");
        assert!(recs.iter().all(|r| r.session_id == "s-1"));
        assert_eq!(recs[0].entrypoint.as_deref(), Some("github-copilot"));
        match &recs[0].event {
            Event::ApiRequest { usage } => {
                assert_eq!(usage.input_tokens, 1200);
                assert_eq!(usage.output_tokens, 340);
                assert_eq!(usage.model.as_deref(), Some("gpt-5"));
                // These conventions have no notion of money: `cost_usd` is
                // Claude Code's own extension, so a Copilot row shows tokens
                // and no dollars rather than a number nobody measured.
                assert_eq!(usage.cost_usd, 0.0);
            }
            other => panic!("expected an api request, got {other:?}"),
        }
        match &recs[1].event {
            Event::ToolFinished {
                tool,
                ok,
                duration_ms,
            } => {
                assert_eq!(tool, "shell");
                assert!(!ok, "status code 2 is an error");
                assert_eq!(*duration_ms, Some(2));
            }
            other => panic!("expected a finished tool, got {other:?}"),
        }
    }

    #[test]
    fn a_span_with_no_session_costs_one_span_and_not_the_batch() {
        let body = serde_json::json!({"resourceSpans": [{"scopeSpans": [{"spans": [
            {"name": "chat x", "attributes": []},
            {"name": "chat y", "attributes": [
                {"key": "session.id", "value": {"stringValue": "s-2"}}
            ]}
        ]}]}]});
        let recs = parse_traces(&serde_json::to_vec(&body).unwrap()).unwrap();
        assert_eq!(recs.len(), 1);
        assert_eq!(recs[0].session_id, "s-2");
    }

    use super::*;
    use serde_json::json;

    fn attr(k: &str, v: serde_json::Value) -> serde_json::Value {
        json!({"key": k, "value": v})
    }

    #[test]
    fn a_fact_the_hook_channel_already_reports_is_not_taken_twice() {
        // `tool_result` and `PostToolUseFailure` describe the same tool call.
        // Taking both counted every failure as two errors, seconds apart.
        let body = json!({"resourceLogs": [{"scopeLogs": [{"logRecords": [{
            "body": {"stringValue": "claude_code.tool_result"},
            "attributes": [
                attr("session.id", json!({"stringValue": "s1"})),
                attr("tool_name", json!({"stringValue": "Bash"})),
                attr("success", json!({"stringValue": "false"}))
            ]
        }]}]}]});
        assert!(
            parse_logs(&serde_json::to_vec(&body).unwrap())
                .unwrap()
                .is_empty()
        );
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
