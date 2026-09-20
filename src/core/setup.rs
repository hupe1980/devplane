//! What a repository's agent configuration grants, read before anybody trusts
//! it.
//!
//! `devplane trust` tells the daemon that headless agents may start in a
//! directory, and a headless agent runs that repository's **own** hooks and MCP
//! servers with no dialog of its own. The command said exactly that and then
//! printed the word `trusted`, which is a consent dialog rather than a gate.
//! This is the evidence: the same files the agent will load, and what they do.
//!
//! Three of the four classes have a measured base rate — of 3,171 public agent
//! setups, 16.0 % carried a confirmed defect: 9.8 % unpinned MCP servers, 3.8 %
//! skills pre-approving the shell, 3.1 % scoped-appearing grants. The fourth, a
//! hook that runs a command out of the repository, is what `trust` has always
//! been about.
//!
//! **It reports and refuses nothing.** A `PreToolUse` hook is a normal thing to
//! ship, and grading repositories would teach people to skip the one prompt
//! here that matters.
//!
//! **It is deliberately shallow.** It does not follow a hook command into the
//! script it names: deciding whether a shell script is hostile is a problem
//! nobody has solved, and a confident tick in front of that case is worse than
//! no tick.

use std::path::{Path, PathBuf};

/// One thing a repository's configuration does when an agent starts in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    /// The file it came from, relative to the repository root.
    pub source: String,
    /// What it is, in one word, for the left column: `hook`, `mcp`, `skill`,
    /// `policy`.
    pub kind: Kind,
    /// The thing itself — a command, a server name, a rule.
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
    /// Skills the repository ships, whether or not any of them is a finding.
    ///
    /// **Counted separately because "nothing to flag" is not "nothing here".**
    /// A skill is reported only when it pre-approves a tool, which is the right
    /// bar for a *finding* — and it left `trust` saying a repository "declares
    /// no skills" about one shipping ten of them. A person deciding whether to
    /// trust a repository wants to know what will load into their agent, not
    /// only which part of it alarmed a scanner.
    pub skills: usize,
    /// Files that exist and could not be parsed. Reported rather than skipped:
    /// a `settings.json` this cannot read is one whose hooks are invisible
    /// here and entirely visible to the agent, which is the worst combination
    /// and the one a silent `continue` used to produce.
    pub unreadable: Vec<String>,
}

impl Setup {
    pub fn is_empty(&self) -> bool {
        self.findings.is_empty() && self.unreadable.is_empty()
    }
}

/// Reads a repository's agent configuration.
///
/// A missing file is not a finding — most repositories have none of these, and
/// "no hooks" is the common case rather than a result. A file that exists and
/// will not parse *is* a finding, in `unreadable`.
pub fn scan(root: &Path) -> Setup {
    let mut out = Setup::default();
    // **One read per file.** Hooks and permissions are two questions about the
    // same document, and asking them separately meant a file that would not
    // parse was reported once per question — which is the "absence is
    // distinguishable" rule failing into double vision instead of silence.
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

/// Allow rules in the agent's own settings that grant an arbitrary program.
///
/// **This reads `.claude/settings.json`, and it used to read `devplane.toml`.**
/// The old version analysed a key that has approved nothing since the approval
/// path was deleted, so it warned about an over-grant that could not happen —
/// while the file where such a rule genuinely does grant was scanned for hooks
/// and not for permissions. Trusting a directory means adopting whatever
/// arrived with somebody else's code, and an accumulated `permissions.allow` is
/// the record of an agent escalating past transient failures.
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
            // The `why` and not the suggestion: this surface is somebody
            // deciding about a repository, not editing its rules.
            detail: wide.why,
        });
    }
}

/// Every skill in the repository that pre-approves a tool for whoever installs
/// it.
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
    // Directory order is filesystem order, which differs between machines and
    // would make this output unstable for no reason.
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

/// Every `command` hook the settings file installs.
///
/// Only `command` hooks: an `http` hook points at a URL rather than at
/// something in this checkout, and a `prompt` hook runs no program. The point
/// of the list is *what will execute on this machine because you trusted this
/// directory*.
pub fn hooks(v: &serde_json::Value, source: &str, out: &mut Vec<Finding>) {
    let Some(events) = v.get("hooks").and_then(|h| h.as_object()) else {
        return;
    };
    // Object order is insertion order in serde_json, which is the file's own
    // order — stable, and the order the person wrote them in.
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

/// Every MCP server the repository declares, and whether it is pinned.
///
/// An unpinned `npx -y <pkg>` is whatever that package's owner published most
/// recently, fetched and executed when an agent starts. It is the largest
/// single class in the study (9.8 %) and the cheapest to see.
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
/// version.
///
/// `npx -y pkg@1.2.3` is pinned; `npx -y pkg` is not. A scoped name keeps its
/// leading `@`, so the version test looks for an `@` after the first
/// character rather than anywhere.
pub fn unpinned_package(command: &str, args: &[&str]) -> Option<String> {
    const LAUNCHERS: &[&str] = &["npx", "bunx", "pnpm", "uvx", "dlx"];
    let program = Path::new(command).file_name()?.to_str()?;
    if !LAUNCHERS.contains(&program) {
        return None;
    }
    // The first argument that is not a flag and not the launcher's own
    // subcommand is the package.
    let pkg = args
        .iter()
        .find(|a| !a.starts_with('-') && **a != "dlx" && **a != "exec")?;
    let versioned = pkg[1..].contains('@');
    (!versioned).then(|| (*pkg).to_string())
}

/// The tools a skill's front matter pre-approves, if any.
///
/// Claude Code's `allowed-tools` in a skill's front matter means those calls
/// are not asked about while the skill runs. A skill that lists `Bash` is
/// handing the shell to whoever installs it, which is the 3.8 % class.
///
/// Only bare tool names count. `Bash(npm test *)` is a scoped grant and is the
/// author doing the right thing; reporting it would make the common good case
/// noisy and teach people to ignore this line.
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
        // The list answers "what executes here because you trusted this
        // directory". A URL and a model are neither.
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
        // Not a fetch-and-run launcher: a local binary is whatever is on the
        // machine already, which trusting this directory did not change.
        assert_eq!(unpinned_package("/usr/local/bin/my-server", &[]), None);
        assert_eq!(unpinned_package("node", &["server.js"]), None);
    }

    /// A repository that ships skills is not a repository that ships nothing.
    ///
    /// **Found by pointing `trust` at this repository** after installing Spec
    /// Kit, which drops ten skills into `.claude/skills/`. None of them
    /// pre-approves a tool, so none is a finding — and the command said the
    /// repository *"declares no hooks, MCP servers or skills"*. The scan was
    /// right and the sentence was false, which is the worse of the two failures
    /// for a gate whose entire job is telling a person what will load into
    /// their agent before they consent to it.
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

        // And a repository with none says none, so the count is a fact rather
        // than a number that is always printed.
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
        // Scoped is the author doing the right thing.
        let scoped = "---\nname: deploy\nallowed-tools: Bash(npm test *)\n---\n\nbody";
        assert_eq!(pre_approved_tools(scoped), None);
        // `Read` is not a command tool, so it is not this line's business.
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
        // Present and broken: the agent will read it and this cannot, which is
        // the one combination that must never be silent.
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
