//! Observation channels.
//!
//! Everything Devplane learns about a session it did not start arrives through
//! one of these, and every one of them is a documented interface:
//!
//! * [`hook`](crate::observe::hook) — Claude Code hooks: lifecycle, blocking, and the policy gate.
//!   Every event runs `devplane hook`, which appends to the store and exits.
//! * [`copilot`](crate::observe::copilot) — GitHub Copilot's hooks, the same way, in its own
//!   vocabulary.
//! * [`otel`](crate::observe::otel) — OpenTelemetry export: per-request cost, tokens and entrypoint.
//! * [`agents_json`](crate::observe::agents_json) — the background roster, authoritative for what the
//!   provider's own daemon supervises.
//! * [`opencode`](crate::observe::opencode) — OpenCode's own event feed, subscribed to.
//! * [`statusline`](crate::observe::statusline) — the optional shim, for rate limits.
//! * [`connect`](crate::observe::connect) — installing and removing the above, carefully.
//! * [`locate`](crate::observe::locate) — finding the `claude` binary, which is routinely not on `PATH`.
//! * [`procs`](crate::observe::procs) — the process table, for leaked agents and for the jobs of a
//!   session no hook has spoken for.
//!
//! Transcript, session and editor-window files under `~/.claude` are not
//! read: their formats are internal and change between releases.

/// A directory a vendor reported, as the filesystem names it.
///
/// One repository is one project whatever path reached it (macOS `/tmp` is
/// `/private/tmp`). Same resolution as [`crate::repo::named`], so an
/// event and its project agree on the spelling.
pub fn canonical_cwd<'de, D>(d: D) -> Result<Option<std::path::PathBuf>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw = <Option<std::path::PathBuf> as serde::Deserialize>::deserialize(d)?;
    Ok(raw.map(|p| crate::repo::named(&p)))
}

pub mod agents_json;
pub mod automode;
pub mod codex;
pub mod connect;
pub mod copilot;
pub mod hook;
pub mod locate;
pub mod opencode;
pub mod otel;
pub mod procs;
pub mod statusline;

#[cfg(test)]
mod tests {
    #[test]
    fn a_linked_directory_is_one_project_whatever_path_reached_it() {
        let real = std::env::temp_dir().join(format!("dp-canon-{}", std::process::id()));
        std::fs::create_dir_all(&real).unwrap();
        let link = real.with_extension("link");
        let _ = std::fs::remove_file(&link);
        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &link).unwrap();
        #[cfg(unix)]
        {
            let body = serde_json::json!({"hook_event_name": "PreToolUse", "session_id": "s", "cwd": link});
            let p: super::hook::HookPayload = serde_json::from_value(body).unwrap();
            assert_eq!(p.cwd.unwrap(), std::fs::canonicalize(&real).unwrap());
        }
        let missing: super::hook::HookPayload = serde_json::from_value(
            serde_json::json!({"hook_event_name": "PreToolUse", "session_id": "s", "cwd": "/no/such/dir"}),
        )
        .unwrap();
        assert_eq!(
            missing.cwd.unwrap(),
            std::path::PathBuf::from("/no/such/dir"),
            "kept as written"
        );
        let _ = std::fs::remove_file(&link);
        let _ = std::fs::remove_dir_all(&real);
    }
}
