//! Whether a change can start in a project, asked of every project before
//! anything is created, so a multi-project start never fails halfway. Pure; the
//! half that reads the disk is [`crate::change::preflight`].

use serde::{Deserialize, Serialize};

/// Why one project cannot take *this* change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum PreflightReason {
    /// Nothing registered answers to that name, and it is not a directory.
    UnknownProject,
    /// Nobody has looked at what this repository would allow an agent to do.
    Untrusted,
    /// Its `devplane.toml` will not load, so its prohibitions are not in force.
    ConfigWillNotLoad,
    /// The chosen agent cannot run here.
    NoSuchAgent,
    /// Uncommitted changes to tracked files.
    DirtyWorktree,
    /// The prompt, spec or task selection cannot be resolved for this project.
    CannotSend,
}

impl PreflightReason {
    pub fn as_str(self) -> &'static str {
        match self {
            PreflightReason::UnknownProject => "unknown_project",
            PreflightReason::Untrusted => "untrusted",
            PreflightReason::ConfigWillNotLoad => "config_will_not_load",
            PreflightReason::NoSuchAgent => "no_such_agent",
            PreflightReason::DirtyWorktree => "dirty_worktree",
            PreflightReason::CannotSend => "cannot_send",
        }
    }

    /// Every reason; a test asserts each is reachable.
    pub const ALL: &'static [PreflightReason] = &[
        PreflightReason::UnknownProject,
        PreflightReason::Untrusted,
        PreflightReason::ConfigWillNotLoad,
        PreflightReason::NoSuchAgent,
        PreflightReason::DirtyWorktree,
        PreflightReason::CannotSend,
    ];

    pub fn says(self) -> &'static str {
        match self {
            PreflightReason::UnknownProject => "no registered project by that name",
            PreflightReason::Untrusted => "not trusted",
            PreflightReason::ConfigWillNotLoad => "its devplane.toml cannot be used",
            PreflightReason::NoSuchAgent => "cannot run that agent",
            PreflightReason::DirtyWorktree => "uncommitted changes",
            PreflightReason::CannotSend => "the prompt cannot be sent here",
        }
    }
}

/// Everything known about one project, with no disk in sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    pub trusted: bool,
    pub config_loads: bool,
    pub agent_available: bool,
    pub worktree_clean: bool,
    pub can_send: bool,
}

/// Whether one project can take this change. Ordered so the refusal that must
/// be fixed first wins (untrusted before dirty).
pub fn refusal(f: &Facts) -> Option<PreflightReason> {
    if !f.trusted {
        return Some(PreflightReason::Untrusted);
    }
    if !f.config_loads {
        return Some(PreflightReason::ConfigWillNotLoad);
    }
    if !f.agent_available {
        return Some(PreflightReason::NoSuchAgent);
    }
    if !f.worktree_clean {
        return Some(PreflightReason::DirtyWorktree);
    }
    if !f.can_send {
        return Some(PreflightReason::CannotSend);
    }
    None
}

/// What will happen in the fresh tree, said before anything starts. Advisory,
/// never a refusal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "note", rename_all = "snake_case")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "PreflightNote", export, export_to = "wire/")
)]
pub enum Note {
    /// `[workspace] setup` will run in the new tree before the agent starts.
    FirstRunInstalls { command: String },
    /// A lockfile is present and no setup is declared: the tree starts empty.
    NoSetupDeclared { lockfiles: Vec<String> },
}

impl Note {
    pub fn says(&self) -> String {
        match self {
            Note::FirstRunInstalls { command } => {
                format!("the first run installs dependencies: `{command}`")
            }
            Note::NoSetupDeclared { lockfiles } => format!(
                "a fresh tree will have no dependencies installed — {} {} present and \
                 [workspace] setup is not declared",
                lockfiles.join(", "),
                if lockfiles.len() == 1 { "is" } else { "are" }
            ),
        }
    }
}

/// Lockfiles whose presence means dependencies need an install step.
pub const LOCKFILES: &[&str] = &[
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lockb",
    "Cargo.lock",
    "go.sum",
    "uv.lock",
    "poetry.lock",
    "requirements.txt",
    "Pipfile.lock",
    "Gemfile.lock",
    "composer.lock",
    "mix.lock",
];

/// The notes for one project. Nothing for a change in place; a shared cargo
/// cache exempts `Cargo.lock`.
pub fn notes_for(
    setup: Option<&str>,
    lockfiles_present: &[String],
    shared: &[String],
    isolates: bool,
) -> Vec<Note> {
    if !isolates {
        return Vec::new();
    }
    if let Some(command) = setup {
        return vec![Note::FirstRunInstalls {
            command: command.to_string(),
        }];
    }
    let cargo_shared = shared.iter().any(|s| s == "cargo");
    let lockfiles: Vec<String> = lockfiles_present
        .iter()
        .filter(|f| !(cargo_shared && f.as_str() == "Cargo.lock"))
        .cloned()
        .collect();
    if lockfiles.is_empty() {
        return Vec::new();
    }
    vec![Note::NoSetupDeclared { lockfiles }]
}

/// One project and what the preflight said about it. A snapshot: the start
/// checks again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "PreflightFinding", export, export_to = "wire/")
)]
pub struct Finding {
    /// What the person asked for: a name, an id or a path.
    pub asked: String,
    /// The registered project's name, or the directory's where none is.
    pub name: String,
    /// The repository root, when the name resolved to one.
    pub root: Option<String>,
    pub refusal: Option<PreflightReason>,
    /// The refusal as one line with its fix.
    pub says: Option<String>,
    /// What the first run in a fresh tree will cost. Never sets `refusal`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<Note>,
}

impl Finding {
    pub fn accepted(&self) -> bool {
        self.refusal.is_none()
    }
}

/// The refusal and its fix, as one line. `detail` (e.g. the parse error)
/// replaces the reason's generic wording.
pub fn refusal_line(name: &str, why: PreflightReason, root: &str, detail: Option<&str>) -> String {
    let fix = match why {
        PreflightReason::UnknownProject => {
            "devplane ls groups by project, and devplane trust <path> adds one".to_string()
        }
        PreflightReason::Untrusted => format!("devplane trust {root}"),
        PreflightReason::ConfigWillNotLoad => format!("devplane check {root}"),
        PreflightReason::NoSuchAgent => "devplane agents lists the ones that can run".to_string(),
        PreflightReason::DirtyWorktree => "commit or stash first".to_string(),
        PreflightReason::CannotSend => String::new(),
    };
    let what = detail.unwrap_or(why.says());
    match fix.is_empty() {
        true => format!("{name}  {what}"),
        false => format!("{name}  {what} — {fix}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> Facts {
        Facts {
            trusted: true,
            config_loads: true,
            agent_available: true,
            worktree_clean: true,
            can_send: true,
        }
    }

    #[test]
    fn an_untrusted_and_dirty_project_is_reported_as_untrusted() {
        let f = Facts {
            trusted: false,
            worktree_clean: false,
            ..facts()
        };
        assert_eq!(refusal(&f), Some(PreflightReason::Untrusted));
        assert_eq!(refusal(&facts()), None);
    }

    #[test]
    fn every_reason_is_one_the_rule_can_produce() {
        let produced = [
            refusal(&Facts {
                trusted: false,
                ..facts()
            }),
            refusal(&Facts {
                config_loads: false,
                ..facts()
            }),
            refusal(&Facts {
                agent_available: false,
                ..facts()
            }),
            refusal(&Facts {
                worktree_clean: false,
                ..facts()
            }),
            refusal(&Facts {
                can_send: false,
                ..facts()
            }),
            // Decided before there are facts: there is no directory to read.
            Some(PreflightReason::UnknownProject),
        ];
        for r in PreflightReason::ALL {
            assert!(produced.contains(&Some(*r)), "{r:?} is never produced");
        }
    }

    #[test]
    fn a_refusal_names_the_command_that_fixes_it() {
        let line = refusal_line(
            "billing",
            PreflightReason::Untrusted,
            "/repos/billing",
            None,
        );
        assert!(line.contains("not trusted"), "{line}");
        assert!(line.contains("devplane trust /repos/billing"), "{line}");
    }

    #[test]
    fn a_fresh_tree_says_what_it_will_install_and_a_tree_in_place_says_nothing() {
        let locks = vec!["Cargo.lock".to_string()];
        assert_eq!(
            notes_for(Some("pnpm i"), &[], &[], true),
            vec![Note::FirstRunInstalls {
                command: "pnpm i".into()
            }]
        );
        assert!(notes_for(None, &locks, &[], false).is_empty());
        assert!(notes_for(None, &locks, &["cargo".into()], true).is_empty());
        assert_eq!(notes_for(None, &locks, &[], true).len(), 1);
    }
}
