//! What a desktop notification carries, and which items get one.
//!
//! Pure: given the inbox, the open asks, the items already announced and what
//! the platform's notifier can do, [`plan`] says which notifications to deliver
//! and which to withdraw, and [`Notice::of`] what each one carries. The edge
//! (the app) only delivers. No clock is read here: `now` is passed in.
//!
//! The rules it holds:
//!
//! - Four kinds notify, as a list rather than a level ([`kinds_that_notify`]).
//! - An item is announced once while it stays open; the key is its ask when it
//!   has one, so a second ask on the same run is a new item.
//! - A notification carries answer actions only for an open permission or
//!   question whose answer can be delivered, and only where the platform shows
//!   actions. A permission offers **Allow** and **Deny** and nothing wider; a
//!   question offers every one of its options or none — never a subset.
//! - An action's meaning is the value bound to its position here, never the
//!   label a platform hands back.
//! - Agent-written text is plain, stripped of control characters and bounded.
//! - Nothing here offers, finishes, archives, marks seen, snoozes, dismisses
//!   or stops: [`NoticeAnswer`] has no variant that could.

use crate::core::ask::{Ask, Kind};
use crate::core::attention::{Action, AttentionItem, AttentionKind};
use crate::core::ids::AskId;
use crate::core::question::Question;
use jiff::Timestamp;
use std::collections::BTreeSet;

/// The four kinds that earn an interruption, and the only four: a question
/// asked, a question abandoned, a permission waiting, and a gate that went red
/// after its agent stopped. A list rather than a level, so raising a kind's
/// inbox level cannot make it buzz.
pub fn kinds_that_notify() -> &'static [AttentionKind] {
    &[
        AttentionKind::Question,
        AttentionKind::QuestionAbandoned,
        AttentionKind::Permission,
        AttentionKind::GateFailed,
    ]
}

/// The longest title a notification shows, in characters.
pub const TITLE_MAX: usize = 64;
/// The longest body.
pub const BODY_MAX: usize = 240;
/// The longest action label; the number is kept first so two never read alike.
pub const LABEL_MAX: usize = 40;

/// What the running platform's notifier can do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct Capability {
    /// How many answer actions one notification can carry; 0 for none.
    pub actions: u8,
    /// Whether a notification can take typed text.
    pub reply: bool,
    /// Whether a delivered notification can be taken back.
    pub withdraw: bool,
    /// Whether a click on the notification's body reaches the app.
    pub click: bool,
}

impl Capability {
    /// Title and body, nothing else: the system notifier, an unbundled build.
    pub const PLAIN: Capability = Capability {
        actions: 0,
        reply: false,
        withdraw: false,
        click: false,
    };
}

/// A permission's answer from a notification: allow this one call, or refuse it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permit {
    Allow,
    Deny,
}

/// What pressing one action answers. Only answers: there is no variant that
/// offers, finishes, archives, marks seen, snoozes, dismisses or stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoticeAnswer {
    /// A permission, once.
    Decision(Permit),
    /// A question's option, by its own value under its own field.
    Option { field: String, value: String },
    /// The person's own words, typed into the notification, under this field.
    Reply { field: String },
}

/// One action on a notification, bound to its position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeAction {
    /// The position; the platform's action id is `<key>:<index>`.
    pub index: u8,
    /// What the button reads: plain and bounded.
    pub label: String,
    pub answers: NoticeAnswer,
}

/// Where a click on the notification's body goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Click {
    /// The answer window, on this ask.
    AnswerWindow(AskId),
    /// The main window, on this query (`ask=…`, `run=…`, or the inbox).
    MainWindow(String),
}

impl Click {
    /// The query the window is opened on.
    pub fn query(&self) -> String {
        match self {
            Click::AnswerWindow(id) => answer_query(id.as_str()),
            Click::MainWindow(q) => q.clone(),
        }
    }
}

/// The answer window's query for one ask.
pub fn answer_query(ask: &str) -> String {
    format!("surface=answer&ask={}", crate::core::text::url_escape(ask))
}

/// One notification, as the host builds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    /// The announcement key: the item's ask id when it has one, else its
    /// attention id. The platform's identifier or tag.
    pub key: String,
    /// The ask its actions answer, when it has one.
    pub ask: Option<AskId>,
    /// The project's name, or `Devplane`.
    pub title: String,
    /// The item's own line, and how many options wait in the window when they
    /// could not all be shown.
    pub body: String,
    /// Empty unless the item is answerable from here; never a subset.
    pub actions: Vec<NoticeAction>,
    pub click: Click,
}

/// What a pass decided: deliver these, take those back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Diff {
    pub announce: Vec<Notice>,
    pub withdraw: Vec<String>,
}

/// The key an item is announced under: its ask, else its attention id. The
/// attention id is per run and kind, so two asks on one run would share it.
pub fn key_of(item: &AttentionItem) -> String {
    match &item.ask {
        Some(ask) => ask.as_str().to_string(),
        None => item.id.as_str().to_string(),
    }
}

/// Which notifications to deliver and which to withdraw.
///
/// `announce`: items of a notifying kind whose key is not yet in `announced`.
/// `withdraw`: keys in `announced` no open item of a notifying kind carries
/// any more — answered, expired, withdrawn by the agent, or resolved. An item
/// that opened and closed between two passes is in neither. Deterministic.
pub fn plan(
    inbox: &[AttentionItem],
    asks: &[Ask],
    announced: &BTreeSet<String>,
    capability: Capability,
    now: Timestamp,
) -> Diff {
    let mut current = BTreeSet::new();
    let mut diff = Diff::default();
    for item in inbox {
        if !kinds_that_notify().contains(&item.kind) {
            continue;
        }
        let key = key_of(item);
        if !current.insert(key.clone()) || announced.contains(&key) {
            continue;
        }
        let ask = ask_of(item, asks);
        diff.announce.push(Notice::of(item, ask, capability, now));
    }
    diff.withdraw = announced
        .iter()
        .filter(|k| !current.contains(*k))
        .cloned()
        .collect();
    diff
}

/// The stored ask an item answers, if it has one.
pub fn ask_of<'a>(item: &AttentionItem, asks: &'a [Ask]) -> Option<&'a Ask> {
    let id = item.ask.as_ref()?;
    asks.iter().find(|a| &a.id == id)
}

/// Until when a watched session's permission is held for an answer, if it is.
fn held_until(ask: &Ask) -> Option<Timestamp> {
    ask.payload
        .get("held_until")
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse().ok())
}

/// The open ask this item's answer can be delivered to, as of `now`: a
/// permission or question, still open, that the inbox itself offers an answer
/// for — a driven run's request, or a watched session's permission while its
/// hold lasts. A watched permission with no hold has no way to reach the
/// agent, and neither has one whose hold ran out.
pub fn deliverable<'a>(
    item: &AttentionItem,
    ask: Option<&'a Ask>,
    now: Timestamp,
) -> Option<&'a Ask> {
    if !matches!(
        item.kind,
        AttentionKind::Permission | AttentionKind::Question
    ) {
        return None;
    }
    let ask = deliverable_ask(ask?, now)?;
    let offered = item.actions.iter().any(|a| {
        matches!(
            a,
            Action::Allow | Action::Deny | Action::Choose | Action::Reply
        )
    });
    offered.then_some(ask)
}

/// The ask alone, as of `now`: still open, unanswered, and — for a watched
/// session's permission — inside its hold. Checked again when an action is
/// pressed, so a press after the hold ran out records nothing.
pub fn deliverable_ask(ask: &Ask, now: Timestamp) -> Option<&Ask> {
    if !ask.is_open() || ask.answer.is_some() {
        return None;
    }
    match held_until(ask) {
        Some(until) if until <= now => None,
        _ => Some(ask),
    }
}

impl Notice {
    /// What one item's notification carries on a platform that can do
    /// `capability`, as of `now`.
    pub fn of(
        item: &AttentionItem,
        ask: Option<&Ask>,
        capability: Capability,
        now: Timestamp,
    ) -> Notice {
        let key = key_of(item);
        let title = plain(&project_name(item), TITLE_MAX);
        let line = plain(&item.title, BODY_MAX);
        let open = deliverable(item, ask, now);
        let (actions, waiting) = match open {
            Some(ask) => answer_actions(ask, capability),
            None => (Vec::new(), None),
        };
        let body = match waiting {
            Some(more) => {
                let room = BODY_MAX.saturating_sub(more.chars().count() + 3);
                format!("{} — {more}", plain(&item.title, room))
            }
            None => line,
        };
        let click = match open {
            Some(ask) => Click::AnswerWindow(ask.id.clone()),
            None => Click::MainWindow(main_query(item)),
        };
        Notice {
            key,
            ask: item.ask.clone(),
            title,
            body,
            actions,
            click,
        }
    }
}

/// The main window's query for an item: its ask, its run, or the inbox.
fn main_query(item: &AttentionItem) -> String {
    let esc = crate::core::text::url_escape;
    match (&item.ask, &item.run_id) {
        (Some(a), _) => format!("ask={}", esc(a.as_str())),
        (None, Some(r)) => format!("run={}", esc(r.as_str())),
        (None, None) => String::new(),
    }
}

/// The project's name — the last segment of its path, which is its id — or
/// the product's.
fn project_name(item: &AttentionItem) -> String {
    match &item.project_id {
        Some(p) => {
            let s = p.as_str().trim_end_matches(['/', '\\']);
            s.rsplit(['/', '\\']).next().unwrap_or(s).to_string()
        }
        None => "Devplane".to_string(),
    }
}

/// The actions an open ask gets, and the sentence for the body when the
/// answer has to be given in the window instead.
fn answer_actions(ask: &Ask, capability: Capability) -> (Vec<NoticeAction>, Option<String>) {
    match ask.kind {
        Kind::Permission => {
            if capability.actions < 2 {
                return (Vec::new(), None);
            }
            // Allow once and deny, whatever wider options the agent offers:
            // a standing grant's scope is read in the window, in full.
            (
                vec![
                    NoticeAction {
                        index: 0,
                        label: "Allow".into(),
                        answers: NoticeAnswer::Decision(Permit::Allow),
                    },
                    NoticeAction {
                        index: 1,
                        label: "Deny".into(),
                        answers: NoticeAnswer::Decision(Permit::Deny),
                    },
                ],
                None,
            )
        }
        Kind::Question => question_actions(&form_of(ask), capability),
    }
}

/// The questions behind an ask, as recorded when it was asked.
fn form_of(ask: &Ask) -> Vec<Question> {
    ask.payload
        .get("form")
        .cloned()
        .and_then(|f| serde_json::from_value(f).ok())
        .unwrap_or_default()
}

/// One field whose options all fit gets them all, numbered in the agent's
/// order, and a reply where both the question and the platform take one.
/// Anything else gets nothing, and the body says what waits in the window.
fn question_actions(
    form: &[Question],
    capability: Capability,
) -> (Vec<NoticeAction>, Option<String>) {
    let [q] = form else {
        let waiting = match form.len() {
            0 => None,
            n => Some(format!("{n} questions — open to answer")),
        };
        return (Vec::new(), waiting);
    };
    let reply = q.custom_field.is_some() && capability.reply;
    let n = q.options.len();
    let fits = n > 0 && n <= usize::from(capability.actions);
    if !fits && !(n == 0 && reply) {
        let waiting = match n {
            0 => "open to answer in your own words".to_string(),
            1 => "1 option — open to answer".to_string(),
            n => format!("{n} options — open to answer"),
        };
        return (Vec::new(), Some(waiting));
    }
    let mut actions: Vec<NoticeAction> = q
        .options
        .iter()
        .enumerate()
        .map(|(i, o)| NoticeAction {
            index: u8::try_from(i).unwrap_or(u8::MAX),
            label: plain(&format!("{}. {}", i + 1, o.label), LABEL_MAX),
            answers: NoticeAnswer::Option {
                field: q.field.clone(),
                value: o.value.clone(),
            },
        })
        .collect();
    if reply {
        actions.push(NoticeAction {
            index: u8::try_from(n).unwrap_or(u8::MAX),
            label: "Reply".into(),
            answers: NoticeAnswer::Reply {
                field: q.field.clone(),
            },
        });
    }
    (actions, None)
}

/// Whether a character may not reach a notification: C0 and C1 controls, and
/// the bidirectional and zero-width marks that make text read other than it is.
fn hidden(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{200B}'..='\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2069}'
                | '\u{FEFF}'
        )
}

/// Agent-written text as a notification may show it: control and direction
/// characters dropped, every run of whitespace one space, at most `max`
/// characters with `…` where it was cut. Never markup: no platform path
/// interprets it, and the Windows path builds it as a text node.
pub fn plain(text: &str, max: usize) -> String {
    let mut out = String::new();
    let mut space = false;
    for c in text.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
            continue;
        }
        if hidden(c) {
            continue;
        }
        if space {
            out.push(' ');
            space = false;
        }
        out.push(c);
    }
    if out.chars().count() <= max {
        return out;
    }
    let mut cut: String = out.chars().take(max.saturating_sub(1)).collect();
    cut.truncate(cut.trim_end().len());
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::ask::{Asked, Deadline};
    use crate::core::attention::ALL_KINDS;
    use crate::core::ids::{AttentionId, ProjectId, RunId};

    fn at(s: &str) -> Timestamp {
        s.parse().unwrap()
    }

    const NOW: &str = "2026-10-03T10:00:00Z";

    /// The richest notifier there is.
    const ALL: Capability = Capability {
        actions: 5,
        reply: true,
        withdraw: true,
        click: true,
    };

    fn item(kind: AttentionKind, ask: Option<&str>, actions: Vec<Action>) -> AttentionItem {
        AttentionItem {
            run_id: Some(RunId::new("run-1")),
            project_id: Some(ProjectId::new("/code/shop")),
            ask: ask.map(AskId::new),
            actions,
            ..AttentionItem::machine(
                kind,
                AttentionId::from(format!("run-1:{}", kind.as_str())),
                "Bash · git push".into(),
                None,
                at(NOW),
            )
        }
    }

    fn ask(id: &str, kind: Kind, payload: serde_json::Value) -> Ask {
        Ask::new(
            AskId::new(id),
            RunId::new("run-1"),
            Asked {
                kind,
                request_id: "7".into(),
                message: "Bash · git push".into(),
                payload,
                at: at(NOW),
                deadline: Deadline::Never,
            },
        )
    }

    fn permission(id: &str) -> (AttentionItem, Ask) {
        (
            item(
                AttentionKind::Permission,
                Some(id),
                vec![Action::Choose, Action::Allow, Action::Deny],
            ),
            ask(
                id,
                Kind::Permission,
                serde_json::json!({"options": [
                    {"id": "allow_once", "label": "Allow", "kind": "allow_once"},
                    {"id": "allow_always", "label": "Always allow", "kind": "allow_always"},
                    {"id": "reject_once", "label": "Reject", "kind": "reject_once"},
                ]}),
            ),
        )
    }

    fn form(options: &[&str], custom: bool) -> serde_json::Value {
        serde_json::json!([{
            "field": "question_0",
            "title": "Keep the legacy route?",
            "options": options.iter().enumerate().map(|(i, l)| serde_json::json!({
                "value": format!("v{i}"), "label": l, "detail": null
            })).collect::<Vec<_>>(),
            "custom_field": custom.then_some("question_0_custom"),
        }])
    }

    fn question(id: &str, form: serde_json::Value) -> (AttentionItem, Ask) {
        (
            item(
                AttentionKind::Question,
                Some(id),
                vec![Action::Choose, Action::Reply],
            ),
            ask(id, Kind::Question, serde_json::json!({"form": form})),
        )
    }

    fn cap(actions: u8, reply: bool) -> Capability {
        Capability {
            actions,
            reply,
            withdraw: true,
            click: true,
        }
    }

    // ── plan ────────────────────────────────────────────────────────────

    #[test]
    fn an_open_item_is_announced_once_and_withdrawn_when_it_leaves() {
        let (p, a) = permission("ask-1");
        let inbox = vec![p.clone()];
        let asks = vec![a];
        let none = BTreeSet::new();
        let d = plan(&inbox, &asks, &none, ALL, at(NOW));
        assert_eq!(d.announce.len(), 1);
        assert_eq!(d.announce[0].key, "ask-1", "keyed by the ask");
        assert!(d.withdraw.is_empty());

        let marked: BTreeSet<String> = ["ask-1".to_string()].into();
        let again = plan(&inbox, &asks, &marked, ALL, at(NOW));
        assert!(again.announce.is_empty(), "already announced");
        assert!(again.withdraw.is_empty());

        let gone = plan(&[], &asks, &marked, ALL, at(NOW));
        assert_eq!(gone.withdraw, vec!["ask-1".to_string()]);
        assert!(gone.announce.is_empty());
    }

    /// Two permissions on one run share the attention id `<run>:permission`,
    /// so the key is the ask: the second is a new item, the first withdrawn.
    #[test]
    fn a_second_ask_on_the_same_run_is_a_new_item() {
        let (p1, a1) = permission("ask-1");
        let (p2, a2) = permission("ask-2");
        assert_eq!(p1.id, p2.id, "the same attention id");
        let marked: BTreeSet<String> = ["ask-1".to_string()].into();
        let d = plan(&[p2], &[a1, a2], &marked, ALL, at(NOW));
        assert_eq!(
            d.announce
                .iter()
                .map(|n| n.key.as_str())
                .collect::<Vec<_>>(),
            ["ask-2"]
        );
        assert_eq!(d.withdraw, vec!["ask-1".to_string()]);
    }

    #[test]
    fn only_the_four_kinds_are_ever_announced() {
        let inbox: Vec<AttentionItem> = ALL_KINDS
            .iter()
            .map(|k| {
                AttentionItem::machine(
                    *k,
                    AttentionId::from(format!("x:{}", k.as_str())),
                    "t".into(),
                    None,
                    at(NOW),
                )
            })
            .collect();
        let d = plan(&inbox, &[], &BTreeSet::new(), ALL, at(NOW));
        let announced: BTreeSet<String> = d.announce.iter().map(|n| n.key.clone()).collect();
        let expected: BTreeSet<String> = kinds_that_notify()
            .iter()
            .map(|k| format!("x:{}", k.as_str()))
            .collect();
        assert_eq!(
            announced, expected,
            "only the four kinds earn an interruption"
        );
    }

    #[test]
    fn the_plan_is_a_function_of_its_inputs() {
        let (p, a) = permission("ask-1");
        let (q, b) = question("ask-2", form(&["Keep it", "Remove it"], false));
        let inbox = vec![p, q];
        let asks = vec![a, b];
        let marked: BTreeSet<String> = ["old".to_string()].into();
        assert_eq!(
            plan(&inbox, &asks, &marked, ALL, at(NOW)),
            plan(&inbox, &asks, &marked, ALL, at(NOW))
        );
    }

    // ── a permission ────────────────────────────────────────────────────

    #[test]
    fn a_driven_permission_offers_allow_once_and_deny_and_nothing_wider() {
        let (p, a) = permission("ask-1");
        let n = Notice::of(&p, Some(&a), ALL, at(NOW));
        assert_eq!(
            n.actions,
            vec![
                NoticeAction {
                    index: 0,
                    label: "Allow".into(),
                    answers: NoticeAnswer::Decision(Permit::Allow)
                },
                NoticeAction {
                    index: 1,
                    label: "Deny".into(),
                    answers: NoticeAnswer::Decision(Permit::Deny)
                },
            ],
            "the agent's *Always allow* stays in the window"
        );
        assert_eq!(n.click, Click::AnswerWindow(AskId::new("ask-1")));
        assert_eq!(n.title, "shop");
        assert_eq!(n.click.query(), "surface=answer&ask=ask-1");
    }

    #[test]
    fn a_watched_permission_is_answerable_only_while_its_hold_lasts() {
        let held = |until: &str| {
            (
                item(
                    AttentionKind::Permission,
                    Some("ask-h"),
                    vec![Action::Choose, Action::Reply],
                ),
                ask(
                    "ask-h",
                    Kind::Permission,
                    serde_json::json!({"held_until": until, "options": [
                        {"id": "allow", "label": "Allow"}, {"id": "deny", "label": "Deny"}
                    ]}),
                ),
            )
        };
        let (i, a) = held("2026-10-03T10:00:30Z");
        let live = Notice::of(&i, Some(&a), ALL, at(NOW));
        assert_eq!(live.actions.len(), 2);
        assert_eq!(live.click, Click::AnswerWindow(AskId::new("ask-h")));

        let (i, a) = held("2026-10-03T09:59:59Z");
        let lapsed = Notice::of(&i, Some(&a), ALL, at(NOW));
        assert!(
            lapsed.actions.is_empty(),
            "the vendor's dialog is asking now"
        );
        assert_eq!(lapsed.click, Click::MainWindow("ask=ask-h".into()));
    }

    #[test]
    fn a_watched_permission_without_a_hold_carries_no_answer() {
        // The vendor owns the dialog: the inbox offers only the way to it.
        let mut i = item(AttentionKind::Permission, None, vec![Action::Attach]);
        i.run_id = Some(RunId::new("sess-9"));
        let n = Notice::of(&i, None, ALL, at(NOW));
        assert!(n.actions.is_empty());
        assert_eq!(n.click, Click::MainWindow("run=sess-9".into()));
    }

    #[test]
    fn a_platform_without_actions_gets_none_and_still_a_click() {
        let (p, a) = permission("ask-1");
        let n = Notice::of(&p, Some(&a), Capability::PLAIN, at(NOW));
        assert!(n.actions.is_empty());
        assert_eq!(n.click, Click::AnswerWindow(AskId::new("ask-1")));
    }

    #[test]
    fn an_answered_ask_offers_nothing() {
        let (p, mut a) = permission("ask-1");
        a.answer(
            serde_json::json!({"permission": "allow"}),
            crate::core::ask::AnswerPath::Cli,
            at(NOW),
        )
        .unwrap();
        assert!(Notice::of(&p, Some(&a), ALL, at(NOW)).actions.is_empty());
    }

    // ── a question ──────────────────────────────────────────────────────

    #[test]
    fn a_question_that_fits_offers_every_option_numbered_in_order() {
        let (q, a) = question(
            "ask-q",
            form(&["Keep it", "Remove it", "Remove it and the tests"], false),
        );
        let n = Notice::of(&q, Some(&a), cap(3, false), at(NOW));
        let labels: Vec<&str> = n.actions.iter().map(|x| x.label.as_str()).collect();
        assert_eq!(
            labels,
            ["1. Keep it", "2. Remove it", "3. Remove it and the tests"]
        );
        for (i, x) in n.actions.iter().enumerate() {
            assert_eq!(usize::from(x.index), i);
            assert_eq!(
                x.answers,
                NoticeAnswer::Option {
                    field: "question_0".into(),
                    value: format!("v{i}")
                },
                "bound to the option's own value, not its label"
            );
        }
        assert_eq!(n.click, Click::AnswerWindow(AskId::new("ask-q")));
    }

    #[test]
    fn a_question_that_does_not_fit_offers_none_and_says_how_many_wait() {
        let (q, a) = question("ask-q", form(&["a", "b", "c", "d"], false));
        let n = Notice::of(&q, Some(&a), cap(3, false), at(NOW));
        assert!(n.actions.is_empty(), "never a subset");
        assert!(n.body.ends_with("4 options — open to answer"), "{}", n.body);
        assert_eq!(n.click, Click::AnswerWindow(AskId::new("ask-q")));

        let (q, a) = question(
            "ask-q",
            form(&["1", "2", "3", "4", "5", "6", "7", "8", "9"], false),
        );
        let n = Notice::of(&q, Some(&a), ALL, at(NOW));
        assert!(n.actions.is_empty());
        assert!(n.body.ends_with("9 options — open to answer"));
    }

    #[test]
    fn several_questions_at_once_are_answered_in_the_window() {
        let two = serde_json::json!([
            {"field": "q0", "title": "A?", "options": [{"value": "y", "label": "Yes", "detail": null}], "custom_field": null},
            {"field": "q1", "title": "B?", "options": [{"value": "n", "label": "No", "detail": null}], "custom_field": null},
        ]);
        let (q, a) = question("ask-q", two);
        let n = Notice::of(&q, Some(&a), ALL, at(NOW));
        assert!(n.actions.is_empty());
        assert!(n.body.ends_with("2 questions — open to answer"));
    }

    #[test]
    fn free_text_is_a_reply_only_where_the_platform_takes_one() {
        let (q, a) = question("ask-q", form(&["Keep it", "Remove it"], true));
        let n = Notice::of(&q, Some(&a), cap(4, true), at(NOW));
        assert_eq!(n.actions.len(), 3);
        assert_eq!(
            n.actions[2].answers,
            NoticeAnswer::Reply {
                field: "question_0".into()
            }
        );
        assert_eq!(n.actions[2].index, 2);

        // No reply on the platform: the options still fit, the words wait.
        let n = Notice::of(&q, Some(&a), cap(3, false), at(NOW));
        assert_eq!(n.actions.len(), 2);

        // Free text only: a reply where one exists, else the window.
        let (q, a) = question("ask-q", form(&[], true));
        assert_eq!(
            Notice::of(&q, Some(&a), cap(4, true), at(NOW))
                .actions
                .len(),
            1
        );
        let n = Notice::of(&q, Some(&a), cap(3, false), at(NOW));
        assert!(n.actions.is_empty());
        assert!(n.body.ends_with("open to answer in your own words"));
    }

    #[test]
    fn duplicate_labels_stay_distinct_by_number() {
        let (q, a) = question("ask-q", form(&["Same", "Same"], false));
        let n = Notice::of(&q, Some(&a), ALL, at(NOW));
        assert_eq!(n.actions[0].label, "1. Same");
        assert_eq!(n.actions[1].label, "2. Same");
        assert_ne!(n.actions[0].answers, n.actions[1].answers);
    }

    /// Markup, control and direction characters, a fake `Allow`, and ten
    /// thousand characters: plain, bounded, and bound by position.
    #[test]
    fn agent_text_is_plain_bounded_and_never_decides_the_answer() {
        let long = "x".repeat(10_000);
        let labels = [
            long.as_str(),
            "<b>bold</b>",
            "evil\u{202E}txt\u{0007}\u{0000}",
            "Allow",
        ];
        let (q, a) = question("ask-q", form(&labels, false));
        let mut q = q;
        q.title = format!("{}\u{001B}[31m red\n\nnext", "y".repeat(500));
        let n = Notice::of(&q, Some(&a), ALL, at(NOW));
        assert_eq!(n.actions.len(), 4);
        for x in &n.actions {
            assert!(x.label.chars().count() <= LABEL_MAX, "{}", x.label);
            assert!(!x.label.chars().any(hidden), "{:?}", x.label);
        }
        assert!(n.actions[0].label.ends_with('…'));
        assert_eq!(
            n.actions[1].label, "2. <b>bold</b>",
            "shown, not interpreted"
        );
        assert_eq!(n.actions[2].label, "3. eviltxt");
        assert_eq!(
            n.actions[3].answers,
            NoticeAnswer::Option {
                field: "question_0".into(),
                value: "v3".into()
            },
            "an option reading `Allow` is the fourth option, nothing more"
        );
        assert!(n.body.chars().count() <= BODY_MAX);
        assert!(!n.body.chars().any(hidden));
        assert!(!n.body.contains('\n'));
        assert!(n.title.chars().count() <= TITLE_MAX);
    }

    #[test]
    fn plain_text_collapses_and_cuts() {
        assert_eq!(plain("  a \n\t b  ", 10), "a b");
        assert_eq!(plain("abcdef", 4), "abc…");
        assert_eq!(plain("ab\u{200B}c", 10), "abc");
        assert_eq!(plain("", 4), "");
    }

    // ── nothing here decides for the person ────────────────────────────

    /// Every kind, every action it can carry, the most capable platform:
    /// answer actions appear only on an open permission or question with an
    /// ask, and each answers — never offers, finishes, archives, marks seen,
    /// snoozes, dismisses or stops.
    #[test]
    fn no_notification_of_any_kind_does_more_than_answer() {
        let every_action = vec![
            Action::Choose,
            Action::Allow,
            Action::Deny,
            Action::Reply,
            Action::Attach,
            Action::Open,
            Action::OpenPr,
            Action::OpenIssue,
            Action::Retry,
            Action::Review,
            Action::Resume,
            Action::TellRun,
            Action::AcceptDrift,
            Action::Answer,
            Action::Snooze,
        ];
        let (_, perm) = permission("ask-p");
        let (_, ques) = question("ask-q", form(&["a", "b"], true));
        for kind in ALL_KINDS {
            for (ask_id, the_ask) in [
                (None, None),
                (Some("ask-p"), Some(&perm)),
                (Some("ask-q"), Some(&ques)),
            ] {
                let i = item(*kind, ask_id, every_action.clone());
                let n = Notice::of(&i, the_ask, ALL, at(NOW));
                let answerable =
                    matches!(kind, AttentionKind::Permission | AttentionKind::Question)
                        && the_ask.is_some();
                if !answerable {
                    assert!(n.actions.is_empty(), "{kind:?} carried an action");
                }
                for x in &n.actions {
                    assert!(matches!(
                        x.answers,
                        NoticeAnswer::Decision(_)
                            | NoticeAnswer::Option { .. }
                            | NoticeAnswer::Reply { .. }
                    ));
                }
            }
        }
        // And a ready-to-decide item is never announced at all.
        let ready = AttentionItem::machine(
            AttentionKind::ReadyToDecide,
            AttentionId::from("c:ready_to_decide"),
            "ready".into(),
            None,
            at(NOW),
        );
        assert!(
            plan(&[ready], &[], &BTreeSet::new(), ALL, at(NOW))
                .announce
                .is_empty()
        );
    }
}
