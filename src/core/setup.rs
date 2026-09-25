//! What a repository's agent configuration grants, read before `devplane
//! trust`: a headless agent runs the repository's own hooks and MCP servers
//! with no dialog. Reports four classes — command hooks, unpinned MCP servers,
//! skills pre-approving the shell, over-broad allow rules — and refuses
//! nothing. Deliberately shallow: it does not follow a hook into the script it
//! names, since judging a script hostile is unsolved.

use std::path::{Path, PathBuf};

/// One thing a repository's configuration does when an agent starts in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file it came from, relative to the repository root.
    pub source: String,
    pub kind: Kind,
    /// A command, a server name, a rule.
    pub subject: String,
    /// One sentence about what it means. Never advice, never a grade.
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Hook,
    Mcp,
    Skill,
    Policy,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Hook => "hook",
            Kind::Mcp => "mcp",
            Kind::Skill => "skill",
            Kind::Policy => "policy",
        }
    }
}

/// Everything found in one repository, in the order a person should read it.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Setup {
    pub findings: Vec<Finding>,
    /// Skills the repository ships, findings or not: "nothing to flag" is not
    /// "nothing will load".
    pub skills: usize,
    /// Files that exist and could not be parsed: their hooks are invisible
    /// here and fully visible to the agent, so this is never silent.
    pub unreadable: Vec<String>,
}

impl Setup {
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty() && self.unreadable.is_empty()
    }
}

/// Reads a repository's agent configuration. A missing file is not a finding;
/// one that exists and will not parse is, in `unreadable`.
pub fn scan(root: &Path) -> Setup {
    let mut out = Setup::default();
    // One read per file, so an unparsable file is reported once.
    for rel in [".claude/settings.json", ".claude/settings.local.json"] {
        read_json(root, rel, &mut out, |v, findings| {
            hooks(v, rel, findings);
            granted(v, rel, findings);
        });
    }
    read_json(root, ".mcp.json", &mut out, |v, findings| {
        mcp_servers(v, ".mcp.json", findings)
    });
    skills(root, &mut out);
    out
}

/// Allow rules in the agent's settings that grant an arbitrary program.
/// Trusting a directory adopts whatever `permissions.allow` arrived with it.
fn granted(v: &serde_json::Value, rel: &str, findings: &mut Vec<Finding>) {
    let rules: Vec<String> = v
        .get("permissions")
        .and_then(|p| p.get("allow"))
        .and_then(|a| a.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|r| r.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    for wide in crate::core::policy::overbroad(&rules) {
        findings.push(Finding {
            source: rel.to_string(),
            kind: Kind::Policy,
            subject: wide.rule,
            // The `why`, not a suggestion: the reader is deciding, not editing.
            detail: wide.why,
        });
    }
}

/// Counts skills and reports each that pre-approves a tool.
fn skills(root: &Path, out: &mut Setup) {
    let dir = root.join(".claude/skills");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path().join("SKILL.md"))
        .filter(|p| p.is_file())
        .collect();
    paths.sort();
    out.skills = paths.len();
    for path in paths {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(tools) = pre_approved_tools(&text) {
            out.findings.push(Finding {
                source: rel,
                kind: Kind::Skill,
                subject: tools.clone(),
                detail: format!(
                    "this skill pre-approves {tools} for whoever installs it, \
                     so those calls are not asked about"
                ),
            });
        }
    }
}

fn read_json(
    root: &Path,
    rel: &str,
    out: &mut Setup,
    f: impl Fn(&serde_json::Value, &mut Vec<Finding>),
) {
    let path = root.join(rel);
    if !path.is_file() {
        return;
    }
    let Ok(text) = std::fs::read_to_string(&path) else {
        out.unreadable.push(rel.to_string());
        return;
    };
    match serde_json::from_str::<serde_json::Value>(&text) {
        Ok(v) => f(&v, &mut out.findings),
        Err(_) => out.unreadable.push(rel.to_string()),
    }
}

/// Every `command` hook the settings file installs. `http` and `prompt` hooks
/// run no program on this machine, so they are not listed.
pub fn hooks(v: &serde_json::Value, source: &str, out: &mut Vec<Finding>) {
    let Some(events) = v.get("hooks").and_then(|h| h.as_object()) else {
        return;
    };
    // serde_json keeps insertion order: the file's own order.
    for (event, matchers) in events {
        let Some(list) = matchers.as_array() else {
            continue;
        };
        for entry in list {
            let Some(handlers) = entry.get("hooks").and_then(|h| h.as_array()) else {
                continue;
            };
            for h in handlers {
                if h.get("type").and_then(|t| t.as_str()) != Some("command") {
                    continue;
                }
                let Some(cmd) = h.get("command").and_then(|c| c.as_str()) else {
                    continue;
                };
                out.push(Finding {
                    source: source.to_string(),
                    kind: Kind::Hook,
                    subject: cmd.trim().to_string(),
                    detail: format!("runs on every {event} in this repository"),
                });
            }
        }
    }
}

/// Every MCP server the repository declares, and whether it is pinned: an
/// unpinned `npx -y <pkg>` runs whatever was published most recently.
pub fn mcp_servers(v: &serde_json::Value, source: &str, out: &mut Vec<Finding>) {
    let Some(servers) = v.get("mcpServers").and_then(|s| s.as_object()) else {
        return;
    };
    for (name, cfg) in servers {
        let command = cfg.get("command").and_then(|c| c.as_str()).unwrap_or("");
        let args: Vec<&str> = cfg
            .get("args")
            .and_then(|a| a.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
            .unwrap_or_default();
        let line = if args.is_empty() {
            command.to_string()
        } else {
            format!("{command} {}", args.join(" "))
        };
        let detail = match unpinned_package(command, &args) {
            Some(pkg) => format!(
                "`{pkg}` has no version pin, so starting an agent fetches and runs \
                 whatever is published at that name today"
            ),
            None if command.is_empty() => "declared over a URL rather than a command".to_string(),
            None => format!("runs `{line}`"),
        };
        out.push(Finding {
            source: source.to_string(),
            kind: Kind::Mcp,
            subject: name.clone(),
            detail,
        });
    }
}

/// The package a fetch-and-run launcher will install, when it carries no
/// version. A scoped name's leading `@` is not a version.
pub fn unpinned_package(command: &str, args: &[&str]) -> Option<String> {
    const LAUNCHERS: &[&str] = &["npx", "bunx", "pnpm", "uvx", "dlx"];
    let program = Path::new(command).file_name()?.to_str()?;
    if !LAUNCHERS.contains(&program) {
        return None;
    }
    // The first non-flag argument that is not the launcher's subcommand.
    let pkg = args
        .iter()
        .find(|a| !a.starts_with('-') && **a != "dlx" && **a != "exec")?;
    let versioned = pkg[1..].contains('@');
    (!versioned).then(|| (*pkg).to_string())
}

/// The bare command tools a skill's `allowed-tools` pre-approves, if any.
/// A scoped grant like `Bash(npm test *)` is the good case and is not reported.
pub fn pre_approved_tools(markdown: &str) -> Option<String> {
    let (front, _) = crate::core::text::split_frontmatter(markdown);
    let front = front?;
    let line = front
        .lines()
        .find_map(|l| l.trim().strip_prefix("allowed-tools:"))?;
    let bare: Vec<&str> = line
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|t| t.trim().trim_matches(['"', '\'']))
        .filter(|t| !t.is_empty() && !t.contains('('))
        .filter(|t| crate::core::policy::is_command_tool(t))
        .collect();
    (!bare.is_empty()).then(|| bare.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_command_hook_is_reported_with_the_event_it_runs_on() {
        let v = serde_json::json!({
            "hooks": { "PreToolUse": [ { "matcher": "Bash", "hooks": [
                { "type": "command", "command": "./scripts/guard.sh" }
            ] } ] }
        });
        let mut out = Vec::new();
        hooks(&v, ".claude/settings.json", &mut out);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].subject, "./scripts/guard.sh");
        assert!(out[0].detail.contains("PreToolUse"), "{:?}", out[0]);
    }

    #[test]
    fn an_http_or_prompt_hook_is_not_a_thing_this_checkout_runs() {
        // A URL and a model are not programs this checkout runs.
        let v = serde_json::json!({
            "hooks": { "Stop": [ { "hooks": [
                { "type": "http", "url": "http://127.0.0.1:1/x" },
                { "type": "prompt", "prompt": "check it" }
            ] } ] }
        });
        let mut out = Vec::new();
        hooks(&v, "s.json", &mut out);
        assert!(out.is_empty(), "{out:?}");
    }

    #[test]
    fn an_unpinned_fetch_and_run_server_is_named_as_such() {
        assert_eq!(
            unpinned_package("npx", &["-y", "some-server"]),
            Some("some-server".into())
        );
        assert_eq!(
            unpinned_package("npx", &["-y", "@scope/server"]),
            Some("@scope/server".into()),
            "a scoped name is still unpinned"
        );
        assert_eq!(unpinned_package("npx", &["-y", "some-server@1.2.3"]), None);
        assert_eq!(
            unpinned_package("npx", &["-y", "@scope/server@1.2.3"]),
            None,
            "the leading @ of a scope is not a version"
        );
        // Not a fetch-and-run launcher.
        assert_eq!(unpinned_package("/usr/local/bin/my-server", &[]), None);
        assert_eq!(unpinned_package("node", &["server.js"]), None);
    }

    /// A repository that ships skills is not a repository that ships nothing.
    #[test]
    fn skills_that_pre_approve_nothing_are_still_counted() {
        let root = std::env::temp_dir().join(format!("vp-setup-{}", uuid::Uuid::new_v4().simple()));
        let skills = root.join(".claude/skills");
        for name in ["speckit-plan", "speckit-tasks"] {
            std::fs::create_dir_all(skills.join(name)).unwrap();
            std::fs::write(
                skills.join(name).join("SKILL.md"),
                "---\nname: \"{name}\"\ndescription: \"does a thing\"\n---\n\nbody",
            )
            .unwrap();
        }
        let setup = scan(&root);
        assert!(
            setup.findings.is_empty(),
            "nothing here pre-approves a tool, so nothing is a finding"
        );
        assert_eq!(
            setup.skills, 2,
            "but two skills will load, and trust says so"
        );

        // A repository with none says none.
        let bare = std::env::temp_dir().join(format!("vp-setup-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&bare).unwrap();
        assert_eq!(scan(&bare).skills, 0);

        std::fs::remove_dir_all(&root).ok();
        std::fs::remove_dir_all(&bare).ok();
    }

    #[test]
    fn a_skill_that_pre_approves_the_shell_is_reported_and_a_scoped_one_is_not() {
        let bare = "---\nname: deploy\nallowed-tools: Bash, Read\n---\n\nbody";
        assert_eq!(pre_approved_tools(bare).as_deref(), Some("Bash"));
        let scoped = "---\nname: deploy\nallowed-tools: Bash(npm test *)\n---\n\nbody";
        assert_eq!(pre_approved_tools(scoped), None);
        // `Read` is not a command tool.
        let reads = "---\nname: notes\nallowed-tools: Read, Glob\n---\n\nbody";
        assert_eq!(pre_approved_tools(reads), None);
        // No front matter at all.
        assert_eq!(pre_approved_tools("# just a heading\n"), None);
    }

    fn tempdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("vp-setup-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn a_settings_file_that_will_not_parse_is_louder_than_one_that_is_absent() {
        let root = &tempdir("broken");
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        // Absent: nothing to say.
        assert!(scan(root).is_empty());
        // Present and broken: the agent reads it and this cannot.
        std::fs::write(root.join(".claude/settings.json"), "{ not json").unwrap();
        let s = scan(root);
        assert_eq!(s.unreadable, vec![".claude/settings.json".to_string()]);
    }

    #[test]
    fn a_repository_with_none_of_this_says_nothing() {
        let dir = tempdir("empty");
        assert!(
            scan(&dir).is_empty(),
            "the common case must produce no output at all"
        );
    }
}
