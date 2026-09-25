//! OpenCode, read over its own event feed.
//!
//! OpenCode publishes the ending of a question (`question.rejected`), where
//! Claude Code's is derived from two hook events — here it is a fact, on a
//! session Devplane never started, with nothing installed in the agent.
//!
//! `question.rejected` carries only `sessionID` and `requestID`, with
//! `additionalProperties: false`, so its authority is `nobody`: "rejected"
//! must not be read as a person declining. `question.asked` keys the request
//! as `id`, the endings as `requestID`, in the same `^que` namespace.
//!
//! The feed does not replay: each subscription opens with a fresh
//! `server.connected`, so a reconnect cannot duplicate, and a drop is a silent
//! gap — hence reported health.

use crate::core::Event;
use crate::core::event::Choice;
use serde::Deserialize;

/// One event off the feed, in the envelope every variant shares.
#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub properties: serde_json::Value,
}

/// What one feed event says about one session.
#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    /// The OpenCode session this is about.
    pub session: String,
    pub event: Event,
}

/// Reads one event off the feed.
///
/// An unknown variant is `None`, and `None` never ends a subscription or
/// changes a run: the vendor ships new variants regularly.
pub fn read(line: &str) -> Option<Observed> {
    let env: Envelope = serde_json::from_str(line).ok()?;
    let p = &env.properties;
    let session = |key: &str| p.get(key).and_then(|v| v.as_str()).map(str::to_string);

    match env.kind.as_str() {
        // The v2 spelling carries the same fields, so both read the same.
        "question.asked" | "question.v2.asked" => {
            let session = session("sessionID")?;
            // Required, and not carried. Its presence makes an ask with no id
            // malformed rather than half-read. Nothing joins on it (a session
            // is blocked on one request at a time), and it must not go into
            // `request_id`, which means "answerable from here".
            let _request_id = session_id_of(p, "id")?;
            let questions = p.get("questions").and_then(|q| q.as_array())?;
            let first = questions.first()?;
            let question = first.get("question").and_then(|v| v.as_str())?.to_string();
            let options = first
                .get("options")
                .and_then(|o| o.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|o| {
                            let label = o.get("label").and_then(|v| v.as_str())?.to_string();
                            Some(Choice {
                                // No id: an OpenCode option is a label and a
                                // description, and an id would read as
                                // "answerable from here".
                                id: None,
                                label,
                                kind: None,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Observed {
                session,
                event: Event::QuestionAsked {
                    question,
                    options,
                    // Absent: the feed is read-only, so nothing here can
                    // answer an OpenCode question.
                    request_id: None,
                    ask: None,
                    form: None,
                },
            })
        }
        "question.replied" | "question.v2.replied" => Some(Observed {
            session: session("sessionID")?,
            // A person answered; `accept` is the neutral word for it.
            event: Event::QuestionAnswered {
                action: "accept".into(),
            },
        }),
        // The event this channel exists for: `QuestionEnded` with no
        // authority, since the closed schema names none.
        "question.rejected" | "question.v2.rejected" => Some(Observed {
            session: session("sessionID")?,
            event: Event::QuestionEnded {
                request_id: session_id_of(p, "requestID")?,
            },
        }),
        // A permission the vendor is asking a person for (`permission.asked`
        // with patterns, or v2 with `action` and `resources`). Read-only
        // feed, so no request id and no options.
        "permission.asked" | "permission.v2.asked" => {
            let session = session("sessionID")?;
            let _request_id = session_id_of(p, "id")?;
            let what = p
                .get("permission")
                .or_else(|| p.get("action"))
                .and_then(|v| v.as_str())
                .map(str::to_string);
            let over: Vec<String> = p
                .get("patterns")
                .or_else(|| p.get("resources"))
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default();
            let message = match (what, over.is_empty()) {
                (Some(w), true) => Some(w),
                (Some(w), false) => Some(format!("{w} · {}", over.join(", "))),
                (None, false) => Some(over.join(", ")),
                (None, true) => None,
            };
            Some(Observed {
                session,
                event: Event::Blocked {
                    waiting_for: crate::core::event::WaitingFor::Permission,
                    message,
                    request_id: None,
                    ask: None,
                    options: Vec::new(),
                    call: None,
                    context: None,
                },
            })
        }
        // The vendor's own reply word (`once`, `always`, `reject`) as the
        // decision; the decider is the vendor's client, nothing more specific.
        "permission.replied" | "permission.v2.replied" => {
            let session = session("sessionID")?;
            let _request_id = session_id_of(p, "requestID")?;
            let reply = p.get("reply").and_then(|v| v.as_str())?.to_string();
            Some(Observed {
                session,
                event: Event::PermissionDecided {
                    // The feed names no tool on a reply; the reducer reads the
                    // block off the run's pending call.
                    tool: String::new(),
                    decision: reply,
                    by: "opencode".into(),
                    reason: None,
                    context: None,
                },
            })
        }
        "session.idle" => Some(Observed {
            session: session("sessionID")?,
            event: Event::TurnEnded,
        }),
        "session.error" => Some(Observed {
            session: session("sessionID")?,
            event: Event::TurnFailed {
                message: p
                    .get("error")
                    .and_then(|e| e.get("data"))
                    .and_then(|d| d.get("message"))
                    .and_then(|m| m.as_str())
                    .unwrap_or("the session reported an error")
                    .to_string(),
            },
        }),
        // Everything else is ignored without ending the subscription or
        // touching any run.
        _ => None,
    }
}

fn session_id_of(p: &serde_json::Value, key: &str) -> Option<String> {
    p.get(key).and_then(|v| v.as_str()).map(str::to_string)
}

/// One session off `GET /session`.
///
/// Read into the roster's shape, so a session appears before it emits
/// anything. `cost`, `tokens` and `model` are not read: they are levels, and
/// the event model has nowhere to put them yet.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Listed {
    pub id: String,
    #[serde(default)]
    pub directory: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
}

/// The roster, from the body `GET /session` returns.
pub fn roster(body: &str) -> Vec<Listed> {
    serde_json::from_str(body).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ending carries no authority: the closed schema names no decider.
    #[test]
    fn a_rejected_question_ends_without_naming_anybody() {
        let o = read(
            r#"{"id":"evt_1","type":"question.rejected",
                "properties":{"sessionID":"ses_a","requestID":"que_1"}}"#,
        )
        .expect("a rejection is read");
        assert_eq!(o.session, "ses_a");
        match o.event {
            Event::QuestionEnded { request_id } => assert_eq!(request_id, "que_1"),
            other => panic!("a rejection became {other:?}"),
        }
    }

    /// An answered question and an abandoned one produce different records.
    #[test]
    fn an_answer_and_an_abandonment_are_not_the_same_record() {
        let replied = read(
            r#"{"id":"evt_2","type":"question.replied",
                "properties":{"sessionID":"ses_a","requestID":"que_1","answers":[["Keep"]]}}"#,
        )
        .expect("a reply is read");
        let rejected = read(
            r#"{"id":"evt_3","type":"question.rejected",
                "properties":{"sessionID":"ses_a","requestID":"que_1"}}"#,
        )
        .expect("a rejection is read");
        assert_ne!(replied.event, rejected.event);
        assert!(
            matches!(replied.event, Event::QuestionAnswered { ref action } if action == "accept"),
            "a reply is somebody having answered: {:?}",
            replied.event
        );
    }

    /// The ask and the ending are keyed to one namespace under two names.
    #[test]
    fn the_ask_and_its_ending_join_on_one_id() {
        let asked = read(
            r#"{"id":"evt_1","type":"question.asked","properties":{
                "id":"que_7","sessionID":"ses_a",
                "questions":[{"question":"Keep the legacy route?","header":"Route",
                  "options":[{"label":"Keep","description":"Leave it in place"},
                             {"label":"Remove","description":"Delete it"}]}]}}"#,
        )
        .expect("an ask is read");
        let ended = read(
            r#"{"id":"evt_2","type":"question.rejected",
                "properties":{"sessionID":"ses_a","requestID":"que_7"}}"#,
        )
        .expect("an ending is read");

        let Event::QuestionAsked {
            question,
            options,
            request_id,
            ..
        } = &asked.event
        else {
            panic!("not an ask: {:?}", asked.event);
        };
        assert_eq!(question, "Keep the legacy route?");
        assert_eq!(options.len(), 2);
        assert_eq!(options[0].label, "Keep");
        assert!(
            request_id.is_none(),
            "an answerable id was offered for a feed this product can only read"
        );
        assert!(
            options.iter().all(|o| o.id.is_none()),
            "an option was given an id the vendor's protocol does not have, and \
             an id is what every surface reads as *answerable from here*"
        );

        let Event::QuestionEnded {
            request_id: ended_id,
        } = &ended.event
        else {
            panic!("not an ending");
        };
        assert_eq!(ended_id, "que_7", "the ending joined on the wrong field");
    }

    /// An unknown variant is ignored without ending anything.
    #[test]
    fn a_variant_nobody_here_knows_is_ignored() {
        for line in [
            r#"{"id":"evt_1","type":"some.future.thing","properties":{"sessionID":"ses_a"}}"#,
            r#"{"id":"evt_2","type":"catalog.updated","properties":{}}"#,
            "not json at all",
            "{}",
        ] {
            assert!(read(line).is_none(), "{line} was read as something");
        }
    }

    /// A malformed known variant is ignored too, rather than half-read.
    #[test]
    fn a_known_variant_missing_its_fields_is_not_half_read() {
        // No `sessionID`: there is no session to attribute this to.
        assert!(
            read(r#"{"id":"e","type":"question.rejected","properties":{"requestID":"que_1"}}"#)
                .is_none()
        );
        // No `requestID`: nothing to join the ending to.
        assert!(
            read(r#"{"id":"e","type":"question.rejected","properties":{"sessionID":"ses_a"}}"#)
                .is_none()
        );
        // An ask with no questions at all.
        assert!(
            read(r#"{"id":"e","type":"question.asked","properties":{"id":"que_1","sessionID":"ses_a","questions":[]}}"#)
                .is_none()
        );
    }

    /// The roster is read, and only what is ingested is parsed.
    #[test]
    fn the_roster_reads_what_the_listing_event_carries() {
        let list = roster(
            r#"[{"id":"ses_a","directory":"/tmp/x","title":"fix login","cost":0.25,"model":{"id":"m"}},
                {"id":"ses_b","directory":"/tmp/y"}]"#,
        );
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].title.as_deref(), Some("fix login"));
        assert_eq!(list[1].directory.as_deref(), Some("/tmp/y"));
    }

    /// The v2 families read as the first ones, and a permission is a person
    /// being asked.
    #[test]
    fn the_v2_and_permission_families_are_read() {
        let asked = read(
            r#"{"type":"question.v2.asked","properties":{"id":"que_1","sessionID":"ses_a",
                "questions":[{"question":"Keep it?","header":"K","options":[{"label":"Yes","description":""}]}]}}"#,
        )
        .expect("a v2 ask is read");
        assert!(matches!(asked.event, Event::QuestionAsked { .. }));
        assert!(matches!(
            read(r#"{"type":"question.v2.rejected","properties":{"sessionID":"ses_a","requestID":"que_1"}}"#)
                .unwrap()
                .event,
            Event::QuestionEnded { .. }
        ));
        assert!(matches!(
            read(r#"{"type":"question.v2.replied","properties":{"sessionID":"ses_a","requestID":"que_1","answers":[]}}"#)
                .unwrap()
                .event,
            Event::QuestionAnswered { .. }
        ));

        let perm = read(
            r#"{"type":"permission.asked","properties":{"id":"per_1","sessionID":"ses_a",
                "permission":"bash","patterns":["rm -rf build"],"metadata":{},"always":[]}}"#,
        )
        .expect("a permission ask is read");
        match &perm.event {
            Event::Blocked {
                waiting_for,
                message,
                request_id,
                ..
            } => {
                assert_eq!(*waiting_for, crate::core::event::WaitingFor::Permission);
                assert_eq!(message.as_deref(), Some("bash · rm -rf build"));
                assert!(request_id.is_none(), "a read-only feed offered an answer");
            }
            other => panic!("{other:?}"),
        }
        let v2 = read(
            r#"{"type":"permission.v2.asked","properties":{"id":"per_2","sessionID":"ses_a",
                "action":"edit","resources":["/repo/src/main.rs"]}}"#,
        )
        .unwrap();
        assert!(matches!(&v2.event, Event::Blocked { message: Some(m), .. } if m.contains("edit")));

        let replied = read(
            r#"{"type":"permission.replied","properties":{"sessionID":"ses_a","requestID":"per_1","reply":"reject"}}"#,
        )
        .unwrap();
        match &replied.event {
            Event::PermissionDecided { decision, by, .. } => {
                assert_eq!(decision, "reject");
                // Nobody more specific is named: the schema names no decider.
                assert_eq!(by, "opencode");
            }
            other => panic!("{other:?}"),
        }
        // A reply without the vendor's own word is not half-read.
        assert!(
            read(r#"{"type":"permission.replied","properties":{"sessionID":"ses_a","requestID":"per_1"}}"#)
                .is_none()
        );
    }

    /// A roster that will not parse is empty rather than a panic.
    #[test]
    fn an_unreadable_roster_is_empty() {
        assert!(roster("not json").is_empty());
        assert!(roster("{}").is_empty());
    }
}

/// Where an OpenCode server is, when the person told us.
///
///
/// Opt-in, never discovered: connecting to a guessed port would be scanning
/// the machine. Absent means this channel is off.
pub const ENV_SERVER: &str = "DEVPLANE_OPENCODE_URL";

/// How the subscription is going, so quiet is never mistaken for nothing.
///
///
/// The feed does not replay, so a drop is a silent gap; health is reported
/// rather than inferred from quiet.
#[derive(Debug, Clone, PartialEq)]
pub enum Health {
    /// Connected, with when an event last arrived.
    Live { last_event: Option<jiff::Timestamp> },
    /// Not reachable, with when it last was — never "zero sessions".
    Down {
        since: jiff::Timestamp,
        because: String,
    },
}

impl Health {
    /// What a surface says about it. Composed here so the terminal and the
    /// board cannot word one state two ways.
    #[must_use]
    pub fn says(&self) -> String {
        match self {
            Health::Live {
                last_event: Some(t),
            } => {
                format!("watching an OpenCode server — last event {t}")
            }
            Health::Live { last_event: None } => {
                "watching an OpenCode server — nothing has happened yet".into()
            }
            Health::Down { since, because } => format!(
                "the OpenCode server has not been reachable since {since} ({because}). \
                 Sessions on it are not being watched, so this is not `no sessions`."
            ),
        }
    }
}

/// Reads one `text/event-stream` chunk into whatever events it carried.
///
///
/// `data:` lines separated by blank lines, parsed as a pure function so the
/// framing is testable. Returns what it read and how much of the buffer it
/// consumed, so a partial frame is kept for the next chunk.
pub fn drain(buffer: &str) -> (Vec<Observed>, usize) {
    let mut out = Vec::new();
    let mut consumed = 0usize;
    for frame in buffer.split("\n\n") {
        // The last piece is only a frame if the buffer ended on a separator.
        if consumed + frame.len() >= buffer.len() {
            break;
        }
        consumed += frame.len() + 2;
        for line in frame.lines() {
            if let Some(data) = line.strip_prefix("data:")
                && let Some(o) = read(data.trim())
            {
                out.push(o);
            }
        }
    }
    (out, consumed)
}

#[cfg(test)]
mod framing {
    use super::*;

    /// A partial frame is kept, not dropped: a packet split must not lose a
    /// question.
    #[test]
    fn a_frame_split_across_chunks_survives() {
        let whole = concat!(
            r#"data: {"id":"evt_1","type":"question.rejected","properties":{"sessionID":"ses_a","requestID":"que_1"}}"#,
            "\n\n",
            r#"data: {"id":"evt_2","type":"session.idle","properties":{"sessionID":"ses_a"}}"#,
            "\n\n"
        );
        // Split anywhere in the first frame.
        let cut = 40;
        let (first, used) = drain(&whole[..cut]);
        assert!(first.is_empty(), "a half-read frame was acted on");
        assert_eq!(used, 0, "an incomplete frame was consumed");

        let (all, used) = drain(whole);
        assert_eq!(all.len(), 2, "two whole frames");
        assert_eq!(used, whole.len(), "whole frames were left in the buffer");
    }

    /// An unknown variant inside a frame does not stop the ones around it.
    #[test]
    fn one_unreadable_frame_does_not_lose_the_others() {
        let buf = concat!(
            "data: {\"type\":\"some.future.thing\",\"properties\":{}}\n\n",
            r#"data: {"id":"e","type":"question.rejected","properties":{"sessionID":"ses_a","requestID":"que_1"}}"#,
            "\n\n"
        );
        let (out, _) = drain(buf);
        assert_eq!(out.len(), 1, "a known event was lost beside an unknown one");
        assert_eq!(out[0].session, "ses_a");
    }

    /// The two states a surface may report, and neither is *no sessions*.
    #[test]
    fn an_unreachable_server_never_reads_as_quiet() {
        let down = Health::Down {
            since: jiff::Timestamp::now(),
            because: "connection refused".into(),
        };
        let says = down.says();
        assert!(says.contains("not been reachable"), "{says}");
        assert!(
            says.contains("not `no sessions`"),
            "an unreachable server could be read as an empty one: {says}"
        );
        assert!(
            Health::Live { last_event: None }
                .says()
                .contains("nothing has happened yet"),
            "a live server with no events reads as broken"
        );
    }
}

/// Subscribes to an OpenCode server's feed, for as long as the host runs.
///
///
/// Opt-in via [`ENV_SERVER`]. It reconnects; reconnecting cannot duplicate
/// (no replay), and a drop is a silent gap, so [`Health`] is reported.
pub async fn watch(state: crate::host::Shared) {
    let Ok(base) = std::env::var(ENV_SERVER) else {
        return;
    };
    let base = base.trim_end_matches('/').to_string();
    tracing::info!(server = %base, "watching an OpenCode event feed");

    let mut backoff = std::time::Duration::from_secs(1);
    loop {
        // The roster on every connect: a session started during a gap is only
        // visible through the listing.
        seed_roster(&state, &base).await;
        // A clean end of stream is the server going away: reported as down.
        let e = subscribe(&state, &base).await.unwrap_err();
        *state.opencode.lock().await = Health::Down {
            since: jiff::Timestamp::now(),
            because: e.to_string(),
        };
        tracing::warn!(error = %e, "the OpenCode feed dropped");
        tokio::time::sleep(backoff).await;
        // Back off to a minute; the server may have been stopped on purpose.
        backoff = (backoff * 2).min(std::time::Duration::from_secs(60));
    }
}

async fn seed_roster(state: &crate::host::Shared, base: &str) {
    let Ok(res) = reqwest::Client::new()
        .get(format!("{base}/session"))
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    else {
        return;
    };
    let Ok(body) = res.text().await else { return };
    for s in roster(&body) {
        // A session matching no registered project is listed, attributed to
        // none; dropping it would make the board quieter than the machine.
        state
            .ingest(
                crate::core::RunId::new(s.id.clone()),
                // `AgentsJson` is the roster source for any vendor's listing,
                // so source values do not grow with integrations.
                crate::core::event::Source::AgentsJson,
                crate::core::Event::SessionListed {
                    agent_session: s.id.clone(),
                    title: s.title.clone(),
                },
                s.directory.as_deref().map(std::path::PathBuf::from),
                crate::core::RunMode::Observed,
            )
            .await;
    }
}

/// How long the feed may say nothing before the socket is presumed dead.
///
/// A half-open connection (a laptop slept, a server was killed) delivers only
/// silence. Generous, since a reconnect cannot duplicate.
const READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(180);

/// Reads the feed until it ends. Never returns `Ok`: a clean end is the server
/// closing it, reported like a drop.
async fn subscribe(state: &crate::host::Shared, base: &str) -> anyhow::Result<()> {
    use futures_util::StreamExt;
    let res = reqwest::Client::new()
        .get(format!("{base}/event"))
        .send()
        .await?;
    anyhow::ensure!(
        res.status().is_success(),
        "the feed answered {}",
        res.status()
    );
    *state.opencode.lock().await = Health::Live { last_event: None };

    let mut stream = res.bytes_stream();
    let mut buffer = String::new();
    loop {
        let chunk = match tokio::time::timeout(READ_TIMEOUT, stream.next()).await {
            Ok(Some(chunk)) => chunk?,
            Ok(None) => anyhow::bail!("the server closed the feed"),
            Err(_) => anyhow::bail!(
                "nothing arrived for {}s, so the connection is presumed dead",
                READ_TIMEOUT.as_secs()
            ),
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        let (events, used) = drain(&buffer);
        buffer.drain(..used);
        // A frame that never completes must not grow without bound.
        if buffer.len() > 1 << 20 {
            anyhow::bail!("a single frame passed a megabyte without completing");
        }
        for o in events {
            *state.opencode.lock().await = Health::Live {
                last_event: Some(jiff::Timestamp::now()),
            };
            state
                .ingest(
                    crate::core::RunId::new(o.session),
                    crate::core::event::Source::Feed,
                    o.event,
                    None,
                    crate::core::RunMode::Observed,
                )
                .await;
        }
    }
}
