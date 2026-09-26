//! `devplane hook` and `devplane statusline`: the processes a vendor runs on
//! every tool call, with the session blocked on the answer.
//!
//! The verdict is computed here from the machine-wide and project policy files,
//! the answer is flushed to stdout first, and only then are the rows written to
//! the store (spooled if it is locked or will not open). Fails closed: anything
//! unreadable is answered *ask* wherever a rule is in force.

use crate::core::Verdict;
use crate::observe::hook::{HookPayload, PermissionResponse, PreToolUseResponse};
use crate::policy_cache::PolicyCache;
use anyhow::Result;
use serde_json::Value;
use std::io::Write;
use std::path::Path;
use std::time::Duration;

/// Lock wait on the store: short, so a busy store costs the record, not the answer.
const STORE_BUSY: Duration = Duration::from_secs(1);

/// Time allowed for writing down what was already answered; fits inside the
/// vendor's timeout ([`crate::observe::hook::GATE_TIMEOUT_SECS`]).
const RECORD_BUDGET: Duration = Duration::from_secs(2);

// ---------------------------------------------------------------------------
// Who is asking
// ---------------------------------------------------------------------------

/// Variables that say this process runs inside an agent's session: Claude
/// Code's Bash tool, Devplane's own [`crate::driven::RUN_ENV`], Codex's shell
/// and Copilot CLI. The Codex and Copilot names are observed, not documented.
pub const AGENT_SESSION_VARS: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_ENTRYPOINT",
    crate::driven::RUN_ENV,
    "CODEX_SANDBOX",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_THREAD_ID",
    "COPILOT_CLI",
];

/// The variable that places this process inside an agent session, if one does.
pub fn agent_session() -> Option<&'static str> {
    AGENT_SESSION_VARS
        .iter()
        .copied()
        .find(|v| std::env::var_os(v).is_some_and(|x| !x.is_empty()))
}

/// Why an allowing answer from inside an agent session is refused.
///
///
/// This stops an agent approving its own permission via `devplane answer
/// --allow` in its shell. It does not stop one that unsets these variables, nor
/// a direct call to the HTTP API with the token in `~/.devplane/token`.
pub fn refuse_self_answer(var: &str) -> String {
    format!(
        "refused: `{var}` is set, so this is running inside an agent's session, and an agent \
         may not approve its own permission. Answer it from the Devplane window, the inbox in a \
         terminal of your own, or your phone. `--deny` is still accepted here."
    )
}

// ---------------------------------------------------------------------------
// The hook
// ---------------------------------------------------------------------------

/// Set once the payload is known to ask for a decision. A crash after that
/// must refuse, never pass: to the vendor, any exit but 2 lets the call run.
static DECIDING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// What a crash in the hook exits with: 2 — the vendor's *blocked*, its
/// stderr shown — while deciding; silence while observing, where a 2 would
/// block a `Stop`.
fn fail(why: &str) -> ! {
    if DECIDING.load(std::sync::atomic::Ordering::SeqCst) {
        eprintln!("Devplane's gate failed, so the call is refused: {why}");
        std::process::exit(2);
    }
    eprintln!("Devplane's hook failed: {why}");
    std::process::exit(0);
}

/// `devplane hook`: one payload on stdin, one answer on stdout. A panic or an
/// error fails closed while deciding (see [`fail`]).
pub async fn run(
    gate: Option<String>,
    observe: Option<String>,
    event: Option<String>,
    vendor: Option<String>,
) -> Result<()> {
    std::panic::set_hook(Box::new(|info| fail(&info.to_string())));
    if let Err(e) = answer_hook(gate, observe, event, vendor).await {
        fail(&format!("{e:#}"));
    }
    Ok(())
}

async fn answer_hook(
    gate: Option<String>,
    observe: Option<String>,
    event: Option<String>,
    vendor: Option<String>,
) -> Result<()> {
    // Lossy UTF-8: one bad byte is still a payload; a failed read is `None`,
    // never a silence that reads as "nothing to decide".
    let mut raw = Vec::new();
    let body = match std::io::Read::read_to_end(&mut std::io::stdin(), &mut raw) {
        Ok(_) => Some(String::from_utf8_lossy(&raw).into_owned()),
        Err(_) => None,
    };

    match (gate.as_deref(), observe.as_deref()) {
        (Some("copilot"), _) => decide_copilot(body.as_deref()).await,
        _ if vendor.as_deref() == Some(crate::observe::codex::VENDOR) => {
            decide_claude(body.as_deref(), crate::core::event::Source::CodexHook).await
        }
        (_, Some("copilot")) => {
            // An observation: written down, and the answer is silence.
            answer("{}");
            if let (Some(event), Some(Ok(payload))) = (
                event.as_deref(),
                body.as_deref()
                    .map(serde_json::from_str::<crate::observe::copilot::HookPayload>),
            ) && let Some(store) = open_store().await
            {
                let _ = within_budget(crate::observe::copilot::observe_copilot(
                    &store, event, &payload,
                ))
                .await;
            }
            Ok(())
        }
        _ => decide_claude(body.as_deref(), crate::core::event::Source::Hook).await,
    }
}

/// Why a call a person should decide is refused on Codex, whose hooks cannot
/// ask one.
fn codex_refusal(verdict: &Verdict) -> String {
    let why = match verdict {
        Verdict::Ask { rule } => format!("{rule} asks that a person decides this"),
        Verdict::Unresolved { why } => {
            format!("Devplane cannot tell whether a prohibition covers this: {why}")
        }
        _ => String::new(),
    };
    format!(
        "{why}. Codex hooks cannot ask a person, so Devplane refuses it; run it yourself, \
         or change the rule in your policy"
    )
}

/// Writes the answer and flushes it, so the vendor has it before any store
/// work begins.
fn answer(text: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{text}");
    let _ = out.flush();
}

/// The store, opened with a short lock wait. `None` when it will not open.
async fn open_store() -> Option<crate::store::Store> {
    let path = crate::config::db_path().ok()?;
    crate::store::Store::open_waiting(&path, STORE_BUSY)
        .await
        .ok()
}

/// Runs a write within [`RECORD_BUDGET`]; `false` when it failed or ran out.
async fn within_budget<T, E>(f: impl std::future::Future<Output = Result<T, E>>) -> bool {
    matches!(tokio::time::timeout(RECORD_BUDGET, f).await, Ok(Ok(_)))
}

/// Every Claude Code and Codex hook event (same shapes). `PreToolUse` and
/// `PermissionRequest` are decided, the rest observed; the answer goes out
/// before anything is written, and a write failure cannot change it.
async fn decide_claude(body: Option<&str>, source: crate::core::event::Source) -> Result<()> {
    let cache = PolicyCache::from_disk().0;
    let value = body.and_then(|b| serde_json::from_str::<Value>(b).ok());
    let payload = match &value {
        Some(v) => serde_json::from_value::<HookPayload>(v.clone()).map_err(|e| e.to_string()),
        None if body.is_none() => Err("the hook's input could not be read".to_string()),
        None => Err("the hook's input is not JSON".to_string()),
    };
    let payload = match payload {
        Ok(p) => p,
        Err(why) => {
            unreadable_claude(&cache, value.as_ref(), &why);
            return Ok(());
        }
    };
    let deciding = matches!(
        payload.hook_event_name.as_str(),
        "PreToolUse" | "PermissionRequest"
    );
    DECIDING.store(deciding, std::sync::atomic::Ordering::SeqCst);
    if !deciding {
        // An observation: written down, and the answer is silence.
        answer("{}");
        if let Some(store) = open_store().await {
            within_budget(crate::record::observe(&store, &payload, source)).await;
        }
        return Ok(());
    }

    let tool = payload.tool_name.clone().unwrap_or_default();
    let input = payload.tool_input.clone().unwrap_or(Value::Null);
    let pre = payload.hook_event_name == "PreToolUse";
    let verdict = judge(&cache, payload.cwd.as_deref(), &tool, &input);

    // `PreToolUse` fires on every call and carries only a prohibition or a
    // question; `PermissionRequest` means a person was about to be asked, so
    // the full verdict applies and the project may hold it for a person.
    let mut held = None;
    if pre {
        let reply = match (&verdict, source) {
            // Codex parses `ask` and does not honour it: it marks the hook
            // failed and runs the call. A question Codex cannot put to a
            // person is a refusal there, never a silent pass.
            (
                Verdict::Ask { .. } | Verdict::Unresolved { .. },
                crate::core::event::Source::CodexHook,
            ) => crate::observe::hook::PreToolUseResponse::deny(codex_refusal(&verdict)),
            _ => crate::observe::hook::pre_tool_use_reply(&verdict),
        };
        answer(&serde_json::to_string(&reply)?);
    } else {
        // Only `ask` is held; a prohibition never softens into a question.
        if let (Verdict::Ask { .. }, Some(dir)) = (&verdict, &payload.cwd) {
            held = hold_for_a_person(&cache, &payload, dir, &tool, &input).await;
        }
        let reply = match held.as_deref() {
            // The person's own selection, carried — never a verdict of ours.
            Some("allow") => PermissionResponse::carrying_a_persons_allow(),
            Some("deny") => PermissionResponse::deny("denied by the person, from Devplane"),
            // Lapsed, or never held: the vendor shows its own dialog.
            _ => crate::observe::hook::permission_reply(&verdict),
        };
        answer(&serde_json::to_string(&reply)?);
    }
    report(
        body.unwrap_or_default(),
        &verdict,
        &payload.session_id,
        &tool,
        crate::observe::hook::describe_call(&tool, &input),
        // No rule and no answer here: the vendor is asking a person now.
        !pre && verdict.rule().is_none() && held.is_none(),
        source,
    )
    .await;
    Ok(())
}

/// The verdict for one call. With no directory in the payload only the
/// machine-wide rules apply, so one repository's rules never answer another's.
/// A command tool with no command is `Unresolved` wherever a rule governs it.
fn judge(cache: &PolicyCache, dir: Option<&Path>, tool: &str, input: &Value) -> Verdict {
    let verdict = match dir {
        Some(d) => cache.restrictive(d, tool, input),
        None => cache.restrictive_global_only(tool, input),
    };
    if verdict == Verdict::Undecided
        && crate::core::policy::is_command_tool(tool)
        && let Some(field) = crate::core::policy::rule_content_field(tool)
        && !input.get(field).is_some_and(Value::is_string)
        && cache.speaks_about(dir, tool)
    {
        return Verdict::Unresolved {
            why: format!("the call's `{field}` is missing or not text, and a rule governs {tool}"),
        };
    }
    verdict
}

/// A Claude Code or Codex payload that would not parse. A `PreToolUse` (or an
/// unknown event) where any rule is in force is answered *ask*; anything else
/// answers `{}`.
fn unreadable_claude(cache: &PolicyCache, value: Option<&Value>, why: &str) {
    let event = value
        .and_then(|v| v.get("hook_event_name"))
        .and_then(Value::as_str);
    let dir = value
        .and_then(|v| v.get("cwd"))
        .and_then(Value::as_str)
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let gated = matches!(event, None | Some("PreToolUse"));
    if gated && !cache.is_empty_at(dir.as_deref()) {
        let reply = PreToolUseResponse::ask(format!(
            "Devplane could not read this call ({why}) and rules are in force here; a person \
             decides it"
        ));
        answer(&serde_json::to_string(&reply).unwrap_or_else(|_| "{}".into()));
    } else {
        answer("{}");
    }
}

/// Waits as long as the project's hold says for a person's answer. `None` for
/// every other outcome (no hold, no store, timed out), which falls back to the
/// vendor's own dialog. The hold is read from the same policy load as the verdict.
async fn hold_for_a_person(
    cache: &PolicyCache,
    payload: &HookPayload,
    dir: &Path,
    tool: &str,
    input: &Value,
) -> Option<String> {
    let hold = cache.hold(dir)?;
    let store = open_store().await?;
    // With no host up, this process is the only one that can announce the hold.
    let host_up = match crate::client::Client::connect() {
        Ok(c) => c.healthy().await,
        Err(_) => false,
    };
    crate::record::hold(
        &store,
        &payload.session_id,
        dir,
        tool,
        &crate::observe::hook::describe_call(tool, input),
        hold,
        !host_up,
    )
    .await
}

/// GitHub Copilot's `preToolUse`: a different vocabulary, the same policy engine.
async fn decide_copilot(body: Option<&str>) -> Result<()> {
    use crate::observe::copilot::{GateReply, HookPayload};

    // The gate hook is only ever installed on `preToolUse`.
    DECIDING.store(true, std::sync::atomic::Ordering::SeqCst);
    let cache = PolicyCache::from_disk().0;
    let parsed = body.map(serde_json::from_str::<HookPayload>);
    let Some(Ok(payload)) = parsed else {
        // Unreadable: a question wherever a rule is in force, else silence.
        let dir = std::env::current_dir().ok();
        let reply = if cache.is_empty_at(dir.as_deref()) {
            GateReply::undecided()
        } else {
            GateReply::ask(
                "Devplane could not read this call and rules are in force here; a person \
                 decides it",
            )
        };
        answer(&reply.to_json().to_string());
        return Ok(());
    };
    let tool = payload.tool();
    let dir = payload.cwd.as_deref();
    // `toolArgs` is an object or a JSON string of one; anything else is a
    // question wherever a rule speaks about the tool.
    let (input, verdict) = match payload.readable_input() {
        Some(input) => {
            let v = judge(&cache, dir, &tool, &input);
            (input, v)
        }
        None if cache.speaks_about(dir, &tool) => (
            payload.input(),
            Verdict::Unresolved {
                why: format!("the arguments to {tool} are not an object this gate can read"),
            },
        ),
        None => (payload.input(), Verdict::Undecided),
    };
    let reply = match &verdict {
        Verdict::Deny { rule } => GateReply::deny(format!("denied by Devplane policy rule {rule}")),
        Verdict::Ask { rule } => GateReply::ask(format!("{rule} asks that a person decides this")),
        // Never folded into `undecided`: unreadable means a person decides.
        Verdict::Unresolved { why } => GateReply::ask(format!("{why}; a person decides this")),
        Verdict::Undecided => GateReply::undecided(),
    };
    answer(&reply.to_json().to_string());
    report(
        body.unwrap_or_default(),
        &verdict,
        &payload.session_id,
        &tool,
        crate::observe::hook::describe_call(&tool, &input),
        false,
        crate::core::event::Source::CopilotHook,
    )
    .await;
    Ok(())
}

/// Writes the decision and observation to the store. Only if that fails or runs
/// past the budget is a rule-bearing decision spooled for the next host.
async fn report(
    body: &str,
    verdict: &Verdict,
    session: &str,
    tool: &str,
    subject: String,
    blocked: bool,
    source: crate::core::event::Source,
) {
    // `devplane doctor`'s probe: a real verdict, recorded nowhere.
    if session == crate::observe::hook::PROBE_SESSION {
        return;
    }
    let env = crate::core::DecidedEnvelope {
        session: session.to_string(),
        source,
        verdict: verdict.as_str().to_string(),
        rule: verdict.rule().map(str::to_string),
        server_source: None,
        why: verdict.why().map(str::to_string),
        subject,
        tool: tool.to_string(),
        at: Some(jiff::Timestamp::now()),
        late: false,
        blocked,
        payload: serde_json::from_str(body).ok(),
    };
    let written = match open_store().await {
        Some(store) => within_budget(crate::record::decided(&store, env.clone())).await,
        None => false,
    };
    if written {
        return;
    }
    // An observation with no rule is re-derivable from the vendor; only a
    // decision a rule or the matcher made is worth spooling.
    if env.rule.is_some() || env.why.is_some() {
        let mut late = env;
        late.late = true;
        if let Ok(v) = serde_json::to_value(&late) {
            let _ = crate::config::spool_decision(&v);
        }
    }
}

/// The status-line shim: record the sample, then run the user's own status line.
pub async fn statusline(then: Option<String>) -> Result<()> {
    use anyhow::Context;
    let mut raw = Vec::new();
    std::io::Read::read_to_end(&mut std::io::stdin(), &mut raw).ok();
    let body = String::from_utf8_lossy(&raw).into_owned();

    // Best effort: the status line runs on every update.
    if let Ok(payload) = serde_json::from_str::<crate::observe::statusline::StatusPayload>(&body)
        && let Some(store) = open_store().await
    {
        within_budget(crate::record::status(&store, &payload)).await;
    }

    if let Some(cmd) = then {
        use std::process::{Command, Stdio};
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(&cmd)
            .stdin(Stdio::piped())
            .spawn()
            .context("running the wrapped status line")?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(body.as_bytes()).ok();
        }
        child.wait().ok();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_session_markers_include_the_ones_this_product_sets_itself() {
        assert!(AGENT_SESSION_VARS.contains(&crate::driven::RUN_ENV));
        assert!(AGENT_SESSION_VARS.contains(&"CLAUDECODE"));
        let says = refuse_self_answer("CLAUDECODE");
        assert!(says.contains("CLAUDECODE") && says.contains("may not approve its own"));
    }

    #[test]
    fn the_record_budget_fits_inside_the_vendors_timeout() {
        assert!(
            STORE_BUSY + RECORD_BUDGET
                < Duration::from_secs(crate::observe::hook::GATE_TIMEOUT_SECS)
        );
    }
}
