//! The first-run checklist, derived: each step's state is read from what is
//! true on the machine — sessions seen, hooks present and their binary still
//! there, a repository trusted, a `check` declared, a change verified — and
//! never from a remembered tick. Skipping is the window's, laid over this.

use serde::Serialize;

/// What the host found, gathered at the edge and passed in.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Facts {
    /// `(vendor, project)` for each session running now.
    pub sessions: Vec<(String, Option<String>)>,
    /// Vendors whose sessions this host cannot see, each as a sentence.
    pub cannot_see: Vec<String>,
    pub updates: Updates,
    pub agents: Vec<AgentFact>,
    /// Names of trusted projects.
    pub trusted: Vec<String>,
    /// Whether a registered project's remote is on a GitHub host nobody is
    /// signed in to: `Some(host)` when so.
    pub github_unsigned: Option<String>,
    /// The project steps five to seven are about.
    pub project: Option<ProjectFact>,
}

/// Whether Devplane may check for updates.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Updates {
    /// This build has no updater, so there is nothing to ask.
    #[default]
    NotThisBuild,
    NotAsked,
    On,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentFact {
    pub name: String,
    pub connected: bool,
    /// Hooks of ours point at a binary that no longer exists.
    pub gate_off: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectFact {
    pub id: String,
    pub name: String,
    /// `check`'s commands; `Err` when `devplane.toml` does not load.
    pub check: Result<Vec<String>, String>,
    /// The spec layouts detected, as names (`Spec Kit (.specify/)`).
    pub dialects: Vec<String>,
    pub latest: Option<ChangeFact>,
    /// A change here that has read *verified* before (its `check` passed),
    /// by title: the checklist ends once one has, whatever came after.
    pub verified_once: Option<String>,
}

impl Default for ProjectFact {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            check: Ok(Vec::new()),
            dialects: Vec::new(),
            latest: None,
            verified_once: None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeFact {
    pub id: String,
    pub title: String,
    /// When it started, as the host stamped it.
    pub created_at: String,
    /// The state word: `verified`, `in flight`, …
    pub state: String,
    /// The weakened-check qualifier, when there is one.
    pub qualifier: Option<String>,
    /// The latest `check`'s exit code when it did not pass, or the sentence
    /// a command not found reads as.
    pub check_failed: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepId {
    Sessions,
    Updates,
    Connect,
    Trust,
    Github,
    Declare,
    Dialect,
    FirstChange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    Done,
    Next,
    Waiting,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Step {
    pub id: StepId,
    pub state: StepState,
    /// What is true now, in one line.
    pub says: String,
    /// What the step would write, named before it is opened.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writes: Option<String>,
    /// The step is shown and moves on without an input.
    pub advances_without_input: bool,
}

/// The steps in order: seven, and *Sign in to GitHub* after trust only while
/// a registered project needs it. The first that is not done is `next`.
pub fn steps(f: &Facts) -> Vec<Step> {
    let mut out: Vec<(StepId, bool, String, Option<String>, bool)> = Vec::new();

    let sessions = match f.sessions.len() {
        0 => "no session is running here".to_string(),
        n => {
            let named: Vec<String> = f
                .sessions
                .iter()
                .take(3)
                .map(|(v, p)| match p {
                    Some(p) => format!("{v} in {p}"),
                    None => v.clone(),
                })
                .collect();
            let more = if n > 3 {
                format!(" and {} more", n - 3)
            } else {
                String::new()
            };
            format!(
                "{n} session{} running: {}{more}",
                if n == 1 { "" } else { "s" },
                named.join(", ")
            )
        }
    };
    let sessions = match f.cannot_see.first() {
        Some(blind) => format!("{sessions} — {blind}"),
        None => sessions,
    };
    out.push((StepId::Sessions, true, sessions, None, true));

    let (done, says) = match f.updates {
        Updates::NotThisBuild => (
            true,
            "this build has no update check, and makes no update request".to_string(),
        ),
        Updates::NotAsked => (
            false,
            "not asked yet — no update request is made until you answer".into(),
        ),
        Updates::On => (true, "checks for updates".into()),
        Updates::Off => (true, "never checks for updates".into()),
    };
    let writes = (f.updates != Updates::NotThisBuild).then(|| "~/.devplane/app.toml".to_string());
    out.push((StepId::Updates, done, says, writes, false));

    let off: Vec<&str> = f
        .agents
        .iter()
        .filter(|a| a.gate_off)
        .map(|a| a.name.as_str())
        .collect();
    let on: Vec<&str> = f
        .agents
        .iter()
        .filter(|a| a.connected && !a.gate_off)
        .map(|a| a.name.as_str())
        .collect();
    let (done, says) = match (off.as_slice(), on.as_slice()) {
        ([], []) => (
            false,
            "no agent connected — sessions are watched, never gated".to_string(),
        ),
        ([], on) => (true, format!("connected: {}", on.join(", "))),
        (off, _) => (
            false,
            format!(
                "gate off for {} — the binary its hooks run is gone",
                off.join(", ")
            ),
        ),
    };
    out.push((
        StepId::Connect,
        done,
        says,
        Some("the agent's user settings".into()),
        false,
    ));

    let (done, says) = match f.trusted.as_slice() {
        [] => (false, "no repository trusted".to_string()),
        names => (true, format!("trusted: {}", names.join(", "))),
    };
    out.push((StepId::Trust, done, says, None, false));

    if let Some(host) = &f.github_unsigned {
        out.push((
            StepId::Github,
            false,
            format!("not signed in to {host} — issues and pull requests are not read"),
            None,
            false,
        ));
    }

    match &f.project {
        None => {
            for id in [StepId::Declare, StepId::Dialect, StepId::FirstChange] {
                out.push((
                    id,
                    false,
                    "trust a repository first".into(),
                    None,
                    id == StepId::Dialect,
                ));
            }
        }
        Some(p) => {
            let (done, says) = match &p.check {
                Ok(c) if !c.is_empty() => (true, format!("{}: check is {}", p.name, c.join(" · "))),
                Ok(_) => (
                    false,
                    format!(
                        "{}: no gates declared — changes here cannot become verified",
                        p.name
                    ),
                ),
                Err(e) => (
                    false,
                    format!("{}: devplane.toml does not load — {e}", p.name),
                ),
            };
            out.push((
                StepId::Declare,
                done,
                says,
                Some("devplane.toml".into()),
                false,
            ));
            let says = match p.dialects.as_slice() {
                [] => format!("{}: no spec dialect detected — a change needs none", p.name),
                d => format!("{}: {}", p.name, d.join(", ")),
            };
            out.push((StepId::Dialect, true, says, None, true));
            let (done, says) = match (&p.latest, &p.verified_once) {
                (Some(c), _) if c.state == "verified" => (
                    true,
                    match &c.qualifier {
                        Some(q) => format!("{} reads verified · {q}", c.title),
                        None => format!("{} reads verified", c.title),
                    },
                ),
                (_, Some(title)) => (true, format!("{title} reached verified")),
                (latest, None) => match latest {
                    None => (false, "no change yet".to_string()),
                    Some(c) if c.state == "verified" => (
                        true,
                        match &c.qualifier {
                            Some(q) => format!("{} reads verified · {q}", c.title),
                            None => format!("{} reads verified", c.title),
                        },
                    ),
                    Some(c) => (
                        false,
                        match &c.check_failed {
                            Some(why) => format!("{}: not verified — {why}", c.title),
                            None => format!("{}: {}", c.title, c.state),
                        },
                    ),
                },
            };
            out.push((StepId::FirstChange, done, says, None, false));
        }
    }

    let next = out.iter().position(|(_, done, ..)| !done);
    out.into_iter()
        .enumerate()
        .map(|(i, (id, done, says, writes, free))| Step {
            id,
            state: if done {
                StepState::Done
            } else if Some(i) == next {
                StepState::Next
            } else {
                StepState::Waiting
            },
            says,
            writes,
            advances_without_input: free,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(s: &[Step]) -> Vec<StepId> {
        s.iter().map(|s| s.id).collect()
    }

    fn state(s: &[Step], id: StepId) -> StepState {
        s.iter().find(|s| s.id == id).unwrap().state
    }

    #[test]
    fn an_empty_home_lists_seven_steps_with_updates_done_in_a_build_without_an_updater() {
        let s = steps(&Facts::default());
        assert_eq!(
            ids(&s),
            vec![
                StepId::Sessions,
                StepId::Updates,
                StepId::Connect,
                StepId::Trust,
                StepId::Declare,
                StepId::Dialect,
                StepId::FirstChange
            ]
        );
        assert_eq!(state(&s, StepId::Sessions), StepState::Done);
        assert_eq!(state(&s, StepId::Updates), StepState::Done);
        assert_eq!(state(&s, StepId::Connect), StepState::Next);
        assert_eq!(state(&s, StepId::Trust), StepState::Waiting);
        assert!(s[0].says.contains("no session"));
        assert!(s[0].advances_without_input);
        assert!(
            s.iter()
                .find(|x| x.id == StepId::Dialect)
                .unwrap()
                .advances_without_input
        );
    }

    #[test]
    fn a_dangling_hook_is_gate_off_by_name_and_not_done() {
        let s = steps(&Facts {
            agents: vec![AgentFact {
                name: "Claude Code".into(),
                connected: true,
                gate_off: true,
            }],
            ..Default::default()
        });
        let c = s.iter().find(|x| x.id == StepId::Connect).unwrap();
        assert_eq!(c.state, StepState::Next);
        assert!(c.says.contains("gate off for Claude Code"), "{}", c.says);
    }

    #[test]
    fn declare_and_first_change_are_read_from_the_project() {
        let mut f = Facts {
            trusted: vec!["sample".into()],
            agents: vec![AgentFact {
                name: "Claude Code".into(),
                connected: true,
                gate_off: false,
            }],
            project: Some(ProjectFact {
                name: "sample".into(),
                check: Ok(vec![]),
                ..Default::default()
            }),
            ..Default::default()
        };
        let s = steps(&f);
        assert_eq!(state(&s, StepId::Declare), StepState::Next);
        assert!(s.iter().any(|x| x.says.contains("cannot become verified")));
        f.project.as_mut().unwrap().check = Ok(vec!["npm test".into()]);
        f.project.as_mut().unwrap().latest = Some(ChangeFact {
            title: "Say hello".into(),
            state: "in flight".into(),
            check_failed: Some("check exited 1".into()),
            ..Default::default()
        });
        let s = steps(&f);
        assert_eq!(state(&s, StepId::Declare), StepState::Done);
        let first = s.iter().find(|x| x.id == StepId::FirstChange).unwrap();
        assert_eq!(first.state, StepState::Next);
        assert!(first.says.contains("not verified — check exited 1"));
        f.project.as_mut().unwrap().latest = Some(ChangeFact {
            title: "Say hello".into(),
            state: "verified".into(),
            qualifier: Some("1 check weakened".into()),
            ..Default::default()
        });
        let s = steps(&f);
        assert!(s.iter().all(|x| x.state == StepState::Done));
        assert!(
            s.last()
                .unwrap()
                .says
                .contains("verified · 1 check weakened")
        );
    }

    #[test]
    fn github_is_a_step_only_while_somebody_needs_to_sign_in() {
        let s = steps(&Facts {
            github_unsigned: Some("github.com".into()),
            ..Default::default()
        });
        assert_eq!(s.len(), 8);
        assert_eq!(s[4].id, StepId::Github);
        assert_eq!(steps(&Facts::default()).len(), 7);
    }
}
