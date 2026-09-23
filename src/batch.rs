//! Fan-out, where it meets the machine.
//!
//! Everything that decides anything lives in [`crate::core::batch`] and is
//! pure. This module gathers the facts the preflight needs, builds the draft
//! links, and — at a running position — hands each accepted target to the
//! **existing** single-target dispatch.
//!
//! # Why it reuses rather than reimplements
//!
//! A batch that had its own dispatch path would be a second place for a
//! permission to be decided, and the whole point of this feature is that a call
//! inside a fan-out is governed by exactly the rules that govern it outside
//! one, crediting the same rule. Reuse is not tidiness here; it is the
//! mechanism by which the claim stays true.

use crate::core::batch::{Batch, Finding, Kind, Position, PreflightReason, TargetFacts, preflight};
use crate::core::{BatchId, ProjectId};

use std::path::{Path, PathBuf};

/// One project a fan-out may reach.
#[derive(Debug, Clone)]
pub struct Target {
    pub project: ProjectId,
    pub name: String,
    pub root: PathBuf,
    pub trusted: bool,
}

/// The deep link that opens one target with the prompt **typed and not sent**.
///
/// Uses `open_dir` and never `open_repo`: the two are not interchangeable.
/// `open_repo` takes an `owner/name` slug and resolves on the *other* machine,
/// so a fan-out over local projects built with it produces links that open a
/// home directory. This takes an absolute path on *this* one.
///
/// `None` when the link would be one the vendor's handler refuses — a relative
/// path, a `..` segment, a UNC path, or a bidirectional control character that
/// can make a path read as one directory and resolve as another. A link that
/// silently does nothing is worse than a target reported as unopenable.
pub fn draft_link(target: &Target, prompt: &str) -> Option<String> {
    crate::core::deeplink::open_dir(&target.root, prompt)
}

/// Where the vendor's own warning changes shape.
///
/// Below this the warning is one line; above it the vendor includes the
/// character count and tells the person to scroll and review before sending. A
/// fan-out prompt quoting a failing test crosses it easily, so the composer
/// says so once rather than letting six windows each surprise somebody.
pub const LONG_PROMPT: usize = 1_000;

/// Whether the composer should show the prompt's length.
pub fn prompt_is_long(prompt: &str) -> bool {
    prompt.chars().count() > LONG_PROMPT
}

/// Gathers what the preflight needs to know about one target.
///
/// Reuses the trust flag the project registry already holds and the policy
/// cache's own view of whether a `devplane.toml` parses, rather than re-reading
/// either: two readers of one fact eventually disagree, and the one that
/// disagrees quietly is this one.
pub async fn facts_for(
    target: &Target,
    agent_available: bool,
    config_loads: bool,
    would_exceed_ceiling: bool,
) -> TargetFacts {
    let worktree_clean = match crate::git::status(&target.root).await {
        Ok(s) => s.is_clean(),
        // **Unreadable is not clean.** A directory git cannot answer for is one
        // this should not start an agent in, and defaulting to clean would put
        // the reassuring answer on the least known case.
        Err(_) => false,
    };
    TargetFacts {
        trusted: target.trusted,
        worktree_clean,
        agent_available,
        config_loads,
        would_exceed_ceiling,
    }
}

/// The preflight over every target, computed **before anything is written**.
///
/// The ceiling applies only at a running position: `Draft` starts nothing, so a
/// limit on parallel runs has nothing to limit. That exemption is expressed by
/// never setting `would_exceed_ceiling` rather than by a branch inside the pure
/// function, which keeps the pure half a total function of its inputs.
pub async fn preflight_all(
    targets: &[Target],
    position: Position,
    agents: &[String],
    agent: &str,
    ceiling: Option<usize>,
    running_now: usize,
    template: Option<&str>,
) -> Vec<Finding> {
    let at = jiff::Timestamp::now();
    let agent_available = agents.iter().any(|a| a == agent);
    // **Computed once, carried per target.** The finding is a property of the
    // artefact against the portable core, and this product reports portability
    // against the three distribution paths rather than against a vendor — so
    // every target loses the same fields, and the surface still shows it per
    // target because that is where a person is choosing.
    let would_lose = lost_fields(template);
    let mut out = Vec::new();
    let mut accepted_so_far = 0usize;
    for t in targets {
        let over = match (position.starts_anything(), ceiling) {
            (true, Some(max)) => running_now + accepted_so_far >= max,
            _ => false,
        };
        let config_loads = crate::core::ProjectConfig::load(&t.root).is_ok();
        let facts = facts_for(t, agent_available, config_loads, over).await;
        let refusal = preflight(&facts);
        if refusal.is_none() {
            accepted_so_far += 1;
        }
        out.push(Finding {
            project: t.project.clone(),
            refusal,
            would_lose_fields: would_lose.clone(),
            at,
        });
    }
    out
}

/// The fields a chosen artefact would lose on the way out, by name.
///
/// **A warning and never a refusal.** The artefact still works in the tool that
/// wrote it; the documented error is about *leaving* it, which is why this
/// cannot turn a target down and is carried beside the refusal rather than in
/// it.
///
/// Empty for a fan-out that starts from a typed prompt rather than from the
/// library, and empty for an artefact whose frontmatter could not be read —
/// `unread` is the library's own distinction between *nothing will be lost* and
/// *nothing was checked*, and a fan-out may not report the second as the first.
/// It is reported as empty here because the composer's claim is about fields it
/// **found**; `devplane library report` is the surface that carries coverage.
fn lost_fields(template: Option<&str>) -> Vec<String> {
    let Some(name) = template else {
        return Vec::new();
    };
    let Ok(all) = crate::library::list() else {
        return Vec::new();
    };
    let Some(artefact) = all.iter().find(|a| a.name == name) else {
        return Vec::new();
    };
    let report = crate::library::portability_of(artefact);
    if report.unread {
        return Vec::new();
    }
    report.findings.into_iter().map(|f| f.field).collect()
}

/// Records a drafted fan-out.
///
/// **Complete on creation.** It never gains members and never waits for them,
/// and nothing anywhere records whether the person went on to send one of the
/// drafts: the vendor's window is the vendor's, and inferring a send from a
/// later session in that directory is manufactured attribution — a row claiming
/// to know something nobody observed.
pub fn drafted(prompt: &str, by: &str, template: Option<&str>, targets: Vec<Finding>) -> Batch {
    Batch {
        id: BatchId::new(format!("b-{}", uuid::Uuid::now_v7().simple())),
        prompt: prompt.to_string(),
        template: template.map(str::to_string),
        position: Position::Draft,
        kind: Kind::Drafted,
        sent_at: jiff::Timestamp::now(),
        sent_by: by.to_string(),
        targets,
    }
}

/// Records a dispatched fan-out.
pub fn dispatched(
    prompt: &str,
    by: &str,
    template: Option<&str>,
    position: Position,
    targets: Vec<Finding>,
) -> Batch {
    Batch {
        id: BatchId::new(format!("b-{}", uuid::Uuid::now_v7().simple())),
        prompt: prompt.to_string(),
        template: template.map(str::to_string),
        position,
        kind: Kind::Dispatched,
        sent_at: jiff::Timestamp::now(),
        sent_by: by.to_string(),
        targets,
    }
}

/// The links a drafted batch produces, in target order.
pub fn draft_links(batch: &Batch, targets: &[Target]) -> Vec<(String, Option<String>)> {
    batch
        .accepted()
        .filter_map(|f| targets.iter().find(|t| t.project == f.project))
        .map(|t| (t.name.clone(), draft_link(t, &batch.prompt)))
        .collect()
}

/// Turns a refusal into the sentence and the fix a person is shown.
pub fn refusal_line(name: &str, why: PreflightReason, root: &Path) -> String {
    let fix = match why {
        PreflightReason::Untrusted => format!("devplane trust {}", root.display()),
        PreflightReason::ConfigWillNotLoad => format!("devplane check {}", root.display()),
        other => other.fix().to_string(),
    };
    format!("{name}  {} — {fix}", why.says())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn target(name: &str, root: &str, trusted: bool) -> Target {
        Target {
            project: ProjectId::new(root),
            name: name.into(),
            root: PathBuf::from(root),
            trusted,
        }
    }

    /// The link form that matters, and the one that would silently open a home
    /// directory.
    #[test]
    fn a_draft_link_carries_this_machines_path_and_the_prompt() {
        let t = target("api", "/repos/api", true);
        let link = draft_link(&t, "bump deps").expect("an absolute path is linkable");
        assert!(link.starts_with("claude-cli://open?cwd="), "{link}");
        assert!(
            link.contains("%2Frepos%2Fapi") || link.contains("/repos/api"),
            "{link}"
        );
        assert!(link.contains("bump") && link.contains("deps"), "{link}");
    }

    /// A link the vendor's handler would refuse is not built.
    #[test]
    fn a_path_the_handler_refuses_produces_no_link_rather_than_a_dead_one() {
        for bad in ["relative/path", "/repos/../etc", "\\\\unc\\share"] {
            let t = target("x", bad, true);
            assert_eq!(
                draft_link(&t, "p"),
                None,
                "{bad} produced a link that would silently do nothing"
            );
        }
        // A bidirectional control character can make a path read as one
        // directory and resolve as another.
        let sneaky = target("x", "/repos/\u{202e}slairtnederc", true);
        assert_eq!(draft_link(&sneaky, "p"), None);
    }

    #[test]
    fn the_composer_is_told_when_the_prompt_crosses_the_vendors_own_threshold() {
        assert!(!prompt_is_long(&"a".repeat(LONG_PROMPT)));
        assert!(prompt_is_long(&"a".repeat(LONG_PROMPT + 1)));
    }

    /// **A draft records the composition and nothing about what happened next.**
    #[test]
    fn a_drafted_batch_is_complete_on_creation_and_records_no_send() {
        let at = jiff::Timestamp::now();
        let targets = vec![Finding {
            project: ProjectId::new("/repos/api"),
            refusal: None,
            would_lose_fields: vec![],
            at,
        }];
        let b = drafted("bump deps", "hupe", None, targets);
        assert_eq!(b.kind, Kind::Drafted);
        assert_eq!(b.position, Position::Draft);

        // Nothing in the record can express "and then they sent it". Asserted
        // over the serialised form, because a field added later would be the
        // manufactured attribution this refuses.
        let json = serde_json::to_string(&b).unwrap().to_lowercase();
        for inferred in [
            "sent_after",
            "was_sent",
            "followed_through",
            "opened_at",
            "accepted_at",
        ] {
            assert!(!json.contains(inferred), "{inferred} in {json}");
        }
    }

    #[test]
    fn a_refusal_names_the_command_that_fixes_it() {
        let line = refusal_line(
            "billing",
            PreflightReason::Untrusted,
            Path::new("/repos/billing"),
        );
        assert!(line.contains("not trusted"), "{line}");
        assert!(line.contains("devplane trust /repos/billing"), "{line}");
    }

    /// A batch of one takes the same path as six.
    #[test]
    fn a_batch_of_one_is_recorded_exactly_like_a_batch_of_six() {
        let at = jiff::Timestamp::now();
        let finding = |p: &str| Finding {
            project: ProjectId::new(p),
            refusal: None,
            would_lose_fields: vec![],
            at,
        };
        let one = dispatched("p", "me", None, Position::ToGate, vec![finding("/a")]);
        let six = dispatched(
            "p",
            "me",
            None,
            Position::ToGate,
            (0..6).map(|i| finding(&format!("/{i}"))).collect(),
        );
        assert_eq!(one.kind, six.kind);
        assert_eq!(one.position, six.position);
        assert_eq!(one.cost_in_runs(), 1);
        assert_eq!(six.cost_in_runs(), 6);
    }
}
