//! A driven session's permission mode: what the agent advertises, what
//! Devplane may ask it for, and what the change says about it.
//!
//! Devplane asks for exactly one thing, and only when the person configured no
//! mode: the agent's own ask-first value, which sends *more* decisions to the
//! person. A mode the person configured — in the agent's own settings, or as
//! `mode` in `agents.toml` — is kept. Which value asks first, and where an
//! adapter starts when nothing is configured, is recorded per adapter version
//! from that adapter's own output ([`PROFILES`]); nothing here maps one
//! vendor's mode onto another's.

use crate::core::run::{ModeStatement, Reading, Why};

/// How an agent advertises its mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// A session config option of this id (`session/set_config_option`).
    Config(String),
    /// The older session modes (`session/set_mode`).
    Modes,
}

/// What one adapter version advertises, recorded from its own output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeProfile {
    /// The built-in agent id it belongs to.
    pub agent: &'static str,
    /// The `initialize` response's `agentInfo` name and version, exactly.
    pub agent_info: (&'static str, &'static str),
    /// `Some(id)` for a config option, `None` for session modes.
    pub config_id: Option<&'static str>,
    /// Where the adapter opens when nothing is configured.
    pub unconfigured_start: &'static str,
    /// The value that asks the person before acting.
    pub ask_first: Option<&'static str>,
    /// Each advertised value: `None` asks the person, `Some(what)` decides
    /// that without them.
    pub values: &'static [(&'static str, Option<&'static str>)],
}

impl ModeProfile {
    pub fn via(&self) -> Via {
        match self.config_id {
            Some(id) => Via::Config(id.to_string()),
            None => Via::Modes,
        }
    }

    /// How this value reads, and what it decides; *not known* for a value the
    /// recording does not contain.
    pub fn reading(&self, value: &str) -> (Reading, Option<String>) {
        match self.values.iter().find(|(v, _)| *v == value) {
            Some((_, None)) => (Reading::Asks, None),
            Some((_, Some(what))) => (Reading::DecidesWithoutYou, Some(what.to_string())),
            None => (Reading::NotKnown, None),
        }
    }
}

/// One profile per pinned adapter version, each value taken from that
/// version's own advertisement (see `tests/fixtures/acp/`). GitHub Copilot is
/// absent: its advertisement has not been recorded, so it is unprofiled.
pub const PROFILES: &[ModeProfile] = &[
    // claude-agent-acp passes `default` to Claude Code when no
    // `permissions.defaultMode` is set, so any other start was configured.
    // `dontAsk` is accepted but not advertised, so it is not here.
    ModeProfile {
        agent: "claude",
        agent_info: ("@agentclientprotocol/claude-agent-acp", "0.85.1"),
        config_id: Some("mode"),
        unconfigured_start: "default",
        ask_first: Some("default"),
        values: &[
            ("default", None),
            ("acceptEdits", Some("edits")),
            ("plan", None),
            ("auto", Some("a classifier decides")),
            ("bypassPermissions", Some("everything")),
        ],
    },
    // codex-acp starts every session in `agent` (Auto review, where a
    // reviewer model answers approvals) unless `INITIAL_AGENT_MODE` is set.
    ModeProfile {
        agent: "codex",
        agent_info: ("@agentclientprotocol/codex-acp", "2.1.1"),
        config_id: Some("mode"),
        unconfigured_start: "agent",
        ask_first: Some("read-only"),
        values: &[
            ("read-only", None),
            ("workspace-write", Some("workspace edits")),
            ("agent", Some("a reviewer model decides")),
            ("agent-full-access", Some("everything")),
        ],
    },
    // Gemini CLI advertises session modes only. `plan` is advertised only
    // when plan mode is enabled, and is not in the recording.
    ModeProfile {
        agent: "gemini",
        agent_info: ("gemini-cli", "0.62.0"),
        config_id: None,
        unconfigured_start: "default",
        ask_first: Some("default"),
        values: &[
            ("default", None),
            ("autoEdit", Some("edits")),
            ("yolo", Some("everything")),
        ],
    },
];

/// The profile for an agent that named itself so, at exactly this version.
pub fn profile_for(name: &str, version: &str) -> Option<&'static ModeProfile> {
    PROFILES
        .iter()
        .find(|p| p.agent_info.0 == name && p.agent_info.1 == version)
}

/// The mode a set-up response advertised: through which channel, its current
/// value, and every value with the agent's own name for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advertised {
    pub via: Via,
    pub current: String,
    pub values: Vec<(String, String)>,
}

impl Advertised {
    /// Reads a set-up response's `configOptions` and `modes` (as JSON). A
    /// profiled agent is read only through its profile's channel — a missing
    /// config option never falls back to modes. An unprofiled agent is read
    /// from its single select option of category `mode`, else its modes.
    pub fn read(
        config_options: &serde_json::Value,
        modes: &serde_json::Value,
        profile: Option<&ModeProfile>,
    ) -> Option<Self> {
        match profile.map(|p| p.via()) {
            Some(Via::Config(id)) => from_config(config_options, |o| o["id"] == id.as_str()),
            Some(Via::Modes) => from_modes(modes),
            None => {
                let options = config_options.as_array().map(Vec::as_slice).unwrap_or(&[]);
                let mode_selects: Vec<&serde_json::Value> = options
                    .iter()
                    .filter(|o| o["category"] == "mode" && o["type"] == "select")
                    .collect();
                match mode_selects.as_slice() {
                    [one] => from_config(config_options, |o| o["id"] == one["id"]),
                    [] if !options.iter().any(|o| o["category"] == "mode") => from_modes(modes),
                    // Several, or only a boolean one: nothing is read.
                    _ => None,
                }
            }
        }
    }

    /// The agent's name for a value, or the value itself.
    pub fn name_of(&self, value: &str) -> String {
        self.values
            .iter()
            .find(|(v, _)| v == value)
            .map(|(_, n)| n.clone())
            .unwrap_or_else(|| value.to_string())
    }

    pub fn offers(&self, value: &str) -> bool {
        self.values.iter().any(|(v, _)| v == value)
    }

    /// The current value of this mode option in a full option list (a
    /// `session/set_config_option` answer or a `config_option_update`).
    pub fn current_in(&self, config_options: &serde_json::Value) -> Option<String> {
        let Via::Config(id) = &self.via else {
            return None;
        };
        config_options
            .as_array()?
            .iter()
            .find(|o| o["id"] == id.as_str())?["currentValue"]
            .as_str()
            .map(str::to_string)
    }
}

fn from_config(
    options: &serde_json::Value,
    pick: impl Fn(&serde_json::Value) -> bool,
) -> Option<Advertised> {
    let option = options.as_array()?.iter().find(|o| pick(o))?;
    // A boolean option is never requested: Devplane does not declare them.
    if option["type"] != "select" {
        return None;
    }
    let mut values = Vec::new();
    for o in option["options"].as_array()? {
        // Options may be grouped.
        match o["options"].as_array() {
            Some(group) => values.extend(group.iter().filter_map(value_of)),
            None => values.extend(value_of(o)),
        }
    }
    Some(Advertised {
        via: Via::Config(option["id"].as_str()?.to_string()),
        current: option["currentValue"].as_str()?.to_string(),
        values,
    })
}

fn value_of(o: &serde_json::Value) -> Option<(String, String)> {
    let v = o["value"].as_str()?;
    Some((v.to_string(), o["name"].as_str().unwrap_or(v).to_string()))
}

fn from_modes(modes: &serde_json::Value) -> Option<Advertised> {
    let current = modes["currentModeId"].as_str()?;
    Some(Advertised {
        via: Via::Modes,
        current: current.to_string(),
        values: modes["availableModes"]
            .as_array()?
            .iter()
            .filter_map(|m| {
                let id = m["id"].as_str()?;
                Some((id.to_string(), m["name"].as_str().unwrap_or(id).to_string()))
            })
            .collect(),
    })
}

/// What Devplane does at set-up, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModePlan {
    /// The one value to request, if any.
    pub request: Option<String>,
    pub why: Why,
}

impl ModePlan {
    /// The decision table; the first row that matches wins. Its only sources
    /// for a requested value are the profile's ask-first value and the
    /// person's own preference.
    pub fn of(
        profile: Option<&ModeProfile>,
        advertised: Option<&Advertised>,
        preference: Option<&str>,
    ) -> Self {
        let none = |why| ModePlan { request: None, why };
        let Some(ad) = advertised else {
            return none(Why::NoneStated);
        };
        if let Some(p) = preference {
            return match (ad.offers(p), p == ad.current) {
                (false, _) => none(Why::ChoseUnoffered),
                (true, true) => none(Why::YouChose),
                (true, false) => ModePlan {
                    request: Some(p.to_string()),
                    why: Why::YouChose,
                },
            };
        }
        let Some(profile) = profile else {
            return none(Why::Unprofiled);
        };
        if profile.ask_first == Some(ad.current.as_str()) {
            return none(Why::AsksAlready);
        }
        if ad.current != profile.unconfigured_start {
            return none(Why::YouSetIt);
        }
        match profile.ask_first.filter(|a| ad.offers(a)) {
            None => none(Why::NoAskFirst),
            Some(a) => ModePlan {
                request: Some(a.to_string()),
                why: Why::AskedForAskFirst,
            },
        }
    }
}

/// How the one request ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// The agent's value afterwards, which may differ from the one asked for.
    Confirmed(String),
    Refused(String),
    NoAnswer,
}

/// The statement for a session's set-up.
pub fn statement(
    agent: &str,
    profile: Option<&ModeProfile>,
    advertised: Option<&Advertised>,
    plan: &ModePlan,
    outcome: Option<&Outcome>,
    preference: Option<&str>,
) -> ModeStatement {
    let current = match (outcome, advertised) {
        (Some(Outcome::Confirmed(v)), _) => Some(v.clone()),
        (_, Some(ad)) => Some(ad.current.clone()),
        _ => None,
    };
    let (reading, decides) = match (&current, profile) {
        (Some(v), Some(p)) => p.reading(v),
        _ => (Reading::NotKnown, None),
    };
    let (why, detail) = match outcome {
        Some(Outcome::Refused(reason)) => (Why::Refused, Some(reason.clone())),
        Some(Outcome::NoAnswer) => (Why::NoAnswer, None),
        _ if plan.why == Why::ChoseUnoffered => {
            (Why::ChoseUnoffered, preference.map(str::to_string))
        }
        _ => (plan.why, None),
    };
    ModeStatement {
        agent: agent.to_string(),
        name: match (&current, advertised) {
            (Some(v), Some(ad)) => Some(ad.name_of(v)),
            _ => None,
        },
        mode: current,
        reading,
        decides,
        why,
        detail,
    }
}

/// The statement for a mode the agent moved to mid-session.
pub fn moved(
    agent: &str,
    profile: Option<&ModeProfile>,
    advertised: Option<&Advertised>,
    value: &str,
) -> ModeStatement {
    let (reading, decides) = profile
        .map(|p| p.reading(value))
        .unwrap_or((Reading::NotKnown, None));
    ModeStatement {
        agent: agent.to_string(),
        mode: Some(value.to_string()),
        name: Some(
            advertised
                .map(|a| a.name_of(value))
                .unwrap_or_else(|| value.to_string()),
        ),
        reading,
        decides,
        why: Why::Moved,
        detail: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn codex() -> &'static ModeProfile {
        profile_for("@agentclientprotocol/codex-acp", "2.1.1").expect("codex profile")
    }

    fn ad(current: &str, values: &[&str]) -> Advertised {
        Advertised {
            via: Via::Config("mode".into()),
            current: current.into(),
            values: values
                .iter()
                .map(|v| (v.to_string(), v.to_string()))
                .collect(),
        }
    }

    const CODEX: &[&str] = &["read-only", "workspace-write", "agent", "agent-full-access"];

    #[test]
    fn the_plan_table_row_by_row() {
        let p = Some(codex());
        let start = ad("agent", CODEX);
        let row = |prof, a: Option<&Advertised>, pref| ModePlan::of(prof, a, pref);
        // 1: nothing advertised.
        assert_eq!(row(p, None, None).why, Why::NoneStated);
        // 2: a preference not offered.
        let r = row(p, Some(&start), Some("nope"));
        assert_eq!((r.request, r.why), (None, Why::ChoseUnoffered));
        // 3: a preference already current.
        let r = row(p, Some(&start), Some("agent"));
        assert_eq!((r.request, r.why), (None, Why::YouChose));
        // 4: a preference offered and not current.
        let r = row(p, Some(&start), Some("workspace-write"));
        assert_eq!(
            (r.request.as_deref(), r.why),
            (Some("workspace-write"), Why::YouChose)
        );
        // 5: no profile.
        assert_eq!(
            row(None, Some(&start), None),
            ModePlan {
                request: None,
                why: Why::Unprofiled
            }
        );
        // 6: already ask-first.
        assert_eq!(
            row(p, Some(&ad("read-only", CODEX)), None).why,
            Why::AsksAlready
        );
        // 7: a configured start.
        let r = row(p, Some(&ad("agent-full-access", CODEX)), None);
        assert_eq!((r.request, r.why), (None, Why::YouSetIt));
        // 8: ask-first not advertised.
        let r = row(p, Some(&ad("agent", &["agent", "workspace-write"])), None);
        assert_eq!((r.request, r.why), (None, Why::NoAskFirst));
        // 9: asked.
        let r = row(p, Some(&start), None);
        assert_eq!(
            (r.request.as_deref(), r.why),
            (Some("read-only"), Why::AskedForAskFirst)
        );
    }

    #[test]
    fn never_requests_a_deciding_mode_unasked() {
        for profile in PROFILES {
            let values: Vec<&str> = profile.values.iter().map(|(v, _)| *v).collect();
            for current in &values {
                let a = Advertised {
                    via: profile.via(),
                    current: current.to_string(),
                    values: values
                        .iter()
                        .map(|v| (v.to_string(), v.to_string()))
                        .collect(),
                };
                let plan = ModePlan::of(Some(profile), Some(&a), None);
                assert!(
                    plan.request.is_none() || plan.request.as_deref() == profile.ask_first,
                    "{} at {current} would request {:?}",
                    profile.agent,
                    plan.request
                );
                if let Some(r) = &plan.request {
                    assert_eq!(profile.reading(r).0, Reading::Asks);
                    assert!(a.offers(r));
                }
            }
        }
        // An unprofiled agent with no preference is never asked anything.
        assert_eq!(
            ModePlan::of(None, Some(&ad("x", &["x", "y"])), None).request,
            None
        );
    }

    #[test]
    fn config_options_are_read_over_modes_and_only_through_the_profile() {
        let config = json!([
            {"id": "model", "category": "model", "type": "select", "currentValue": "m",
             "options": [{"value": "m", "name": "M"}]},
            {"id": "mode", "category": "mode", "type": "select", "currentValue": "agent",
             "options": [{"value": "read-only", "name": "Read-only"},
                         {"value": "agent", "name": "Auto review"}]}
        ]);
        let modes =
            json!({"currentModeId": "other", "availableModes": [{"id": "other", "name": "O"}]});
        let a = Advertised::read(&config, &modes, Some(codex())).expect("read");
        assert_eq!(a.via, Via::Config("mode".into()));
        assert_eq!(a.current, "agent");
        assert_eq!(a.name_of("agent"), "Auto review");
        // A profiled config agent with no such option never falls back to modes.
        assert_eq!(Advertised::read(&json!([]), &modes, Some(codex())), None);
        // Unprofiled: the single mode select, else modes.
        assert_eq!(
            Advertised::read(&config, &modes, None).unwrap().current,
            "agent"
        );
        assert_eq!(
            Advertised::read(&json!([]), &modes, None).unwrap().via,
            Via::Modes
        );
        // A boolean mode option is not read, and not replaced by modes.
        let boolean =
            json!([{"id": "mode", "category": "mode", "type": "boolean", "currentValue": true}]);
        assert_eq!(Advertised::read(&boolean, &modes, None), None);
        // Gemini's profile reads modes.
        let gemini = profile_for("gemini-cli", "0.62.0").unwrap();
        assert_eq!(
            Advertised::read(&config, &modes, Some(gemini)).unwrap().via,
            Via::Modes
        );
    }

    #[test]
    fn statements_name_the_mode_in_the_agents_words() {
        let a = Advertised {
            via: Via::Config("mode".into()),
            current: "agent".into(),
            values: vec![
                ("read-only".into(), "Read-only".into()),
                ("agent".into(), "Auto review".into()),
            ],
        };
        let plan = ModePlan::of(Some(codex()), Some(&a), None);
        let s = statement(
            "Codex",
            Some(codex()),
            Some(&a),
            &plan,
            Some(&Outcome::Confirmed("read-only".into())),
            None,
        );
        assert_eq!(
            s.sentence(),
            "asked Codex to ask before acting — you set no mode"
        );
        assert_eq!(s.name.as_deref(), Some("Read-only"));
        assert_eq!(s.asks_a_person(), Some(true));

        let s = statement(
            "Codex",
            Some(codex()),
            Some(&a),
            &plan,
            Some(&Outcome::NoAnswer),
            None,
        );
        assert_eq!(
            s.sentence(),
            "asked Codex to ask before acting; it did not answer — it is in Auto review — decides without you: a reviewer model decides"
        );
        let m = moved("Codex", Some(codex()), Some(&a), "agent");
        assert_eq!(
            m.sentence(),
            "Codex moved to Auto review — decides without you: a reviewer model decides"
        );
        let u = moved("Kimi", None, None, "turbo");
        assert_eq!(
            u.sentence(),
            "Kimi moved to turbo — not known whether it asks you"
        );
    }

    /// Every value a profile names is in its adapter's fixture set-up answer.
    #[test]
    fn every_profile_value_is_in_its_recording() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/acp");
        for p in PROFILES {
            let pin = crate::acp::agent::builtin()
                .into_iter()
                .find(|a| a.id == p.agent)
                .expect("a built-in agent");
            let dir = crate::acp::agent::tests::recording_dir(&pin).expect("an npm pin");
            let text = std::fs::read_to_string(root.join(&dir).join("session.jsonl"))
                .unwrap_or_else(|e| panic!("{}: no recording at {dir}: {e}", p.agent));
            let setup = text
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .filter(|e| e["dir"] == "out")
                .map(|e| e["msg"]["result"].clone())
                .find(|r| r.get("sessionId").is_some())
                .unwrap_or_else(|| panic!("{}: no session/new answer in {dir}", p.agent));
            let ad = Advertised::read(&setup["configOptions"], &setup["modes"], Some(p))
                .unwrap_or_else(|| panic!("{}: its recording advertises no mode", p.agent));
            for (v, _) in p.values {
                assert!(ad.offers(v), "{}: `{v}` is not in {dir}", p.agent);
            }
            assert!(ad.offers(p.unconfigured_start), "{}: start", p.agent);
            if let Some(a) = p.ask_first {
                assert!(ad.offers(a), "{}: ask-first", p.agent);
            }
            let info = text
                .lines()
                .filter_map(|l| serde_json::from_str::<serde_json::Value>(l).ok())
                .find_map(|e| e["msg"]["result"].get("agentInfo").cloned())
                .unwrap_or_else(|| panic!("{}: no agentInfo in {dir}", p.agent));
            assert_eq!(
                (info["name"].as_str(), info["version"].as_str()),
                (Some(p.agent_info.0), Some(p.agent_info.1))
            );
        }
    }
}
