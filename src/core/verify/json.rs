//! `devplane verify --json`: one document with everything the summary says and
//! every row it may have folded. Keys are camelCase; none is named `verified`.

use super::{CouldNotRun, Failed, Run, Verdict};
use serde_json::{Value, json};

/// Where the outputs went.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Wrote {
    pub summary: Option<String>,
    pub statement: Option<String>,
    /// Why no statement was written although one was asked for.
    pub statement_withheld: Option<String>,
    /// `--sarif`: where the SARIF log went.
    pub sarif: Option<String>,
}

/// The document.
pub fn json(run: &Run<'_>, wrote: &Wrote) -> Value {
    let v = run.verdict;
    let mut doc = json!({
        "verdict": v.key(),
        "exit": v.exit_code(),
        "says": v.says(),
        "base": run.base.map(|b| json!({
            "asked": b.asked,
            "askedFrom": b.asked_from,
            "resolved": b.resolved,
            "mergeBase": b.merge_base,
            "head": b.head,
            "forgeMerge": b.forge_merge,
        })),
        "tree": match v {
            Verdict::Verified { tree } => Some(tree.clone()),
            _ => run.report.and_then(|r| r.commit.as_ref()).and_then(|c| c.tree.clone()),
        },
        "weakened": {
            "counts": super::summary::counts(run.rows),
            // Every row, never folded: the summary's folds point here.
            "rows": run.rows,
            "truncated": run.truncated.map(|t| json!({
                "filesShown": t.files_shown,
                "filesTotal": t.files_total,
                "command": t.command,
            })),
        },
        "noChange": run.no_change,
        "definition": run.definition.map(|d| json!({
            "from": run.base.map(|b| b.merge_base.clone()),
            "fileBlob": d.file_blob,
            "check": d.check,
            "timeoutSecs": d.timeout_secs,
            "changeDeclares": d.change_declares,
        })),
        "commands": run.report.map(|r| r.commands.iter().map(|c| json!({
            "command": c.command,
            "outcome": c.outcome,
            "says": crate::core::certificate::outcome_sentence(c),
            "durationMs": c.duration_ms,
            "failures": c.failures,
            "outputTail": c.output_tail,
            "outputBytes": c.output_bytes,
        })).collect::<Vec<_>>()).unwrap_or_default(),
        "treeMoved": match v {
            Verdict::Failed { why: Failed::TreeMoved { paths, more } } => json!({ "paths": paths, "more": more }),
            _ => Value::Null,
        },
        // The statement's shape, beside the sentences the summary shows.
        "baseChecks": run.base_checks.json(),
        "baseChecksSays": run.base_checks.says(),
        "tests": run.base_checks.counts(),
        "notDone": run.not_done,
        "wrote": {
            "summary": wrote.summary,
            "statement": wrote.statement,
            "statementWithheld": wrote.statement_withheld,
            "sarif": wrote.sarif,
        },
        "annotations": run.annotations.map(|a| json!({
            "printed": a.printed,
            "leftOut": a.left_out,
        })),
    });
    match v {
        Verdict::Failed { why } => {
            doc["failed"] = json!(match why {
                Failed::Command { .. } => "command",
                Failed::TreeMoved { .. } => "tree_moved",
                Failed::TreeUnreadable { .. } => "tree_unreadable",
            });
        }
        Verdict::CouldNotRun { why } => {
            doc["reason"] = json!(why.key());
            match why {
                CouldNotRun::BaseUnresolvable { tried, .. } => doc["tried"] = json!(tried),
                CouldNotRun::NoSharedHistory { shallow, .. } => doc["shallow"] = json!(shallow),
                CouldNotRun::CouldNotWrite { path, why } => {
                    doc["path"] = json!(path);
                    doc["why"] = json!(why);
                }
                CouldNotRun::ConfigUnreadableAtBase { why, .. }
                | CouldNotRun::DiffUnreadable { why, .. } => doc["why"] = json!(why),
                _ => {}
            }
        }
        Verdict::Verified { .. } => {}
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::super::tests::{report, same};
    use super::super::*;
    use super::*;
    use crate::core::change::CommandResult;
    use crate::core::review::{WeakKind, Weakened};

    fn keys(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::Object(m) => {
                for (k, v) in m {
                    out.push(k.clone());
                    keys(v, out);
                }
            }
            Value::Array(a) => a.iter().for_each(|v| keys(v, out)),
            _ => {}
        }
    }

    #[test]
    fn the_document_carries_every_row_and_no_forbidden_key() {
        let rows: Vec<Weakened> = (0..3000)
            .map(|i| Weakened {
                path: format!("tests/{i}.test.js"),
                why: "adds a skip marker `it.skip(`".into(),
                kind: WeakKind::Skip,
                matched: "it.skip(".into(),
            })
            .collect();
        let r = report(vec![CommandResult::exited("npm test", 0)]);
        let verdicts = [
            Verdict::of(&r, &same("t")),
            Verdict::Failed {
                why: Failed::TreeMoved {
                    paths: vec!["g".into()],
                    more: 0,
                },
            },
            Verdict::CouldNotRun {
                why: CouldNotRun::NoCheck { at: "a".into() },
            },
        ];
        for v in &verdicts {
            let bc = BaseChecks::NotReached;
            let doc = json(
                &Run {
                    base: None,
                    definition: None,
                    rows: &rows,
                    truncated: None,
                    no_change: false,
                    report: Some(&r),
                    verdict: v,
                    base_checks: &bc,
                    not_done: &[],
                    annotations: None,
                },
                &Wrote::default(),
            );
            assert_eq!(doc["exit"], v.exit_code());
            assert_eq!(doc["verdict"], v.key());
            assert_eq!(doc["weakened"]["rows"].as_array().unwrap().len(), 3000);
            let mut all = Vec::new();
            keys(&doc, &mut all);
            for bad in ["verified", "score", "rate", "percent"] {
                assert!(!all.iter().any(|k| k == bad), "key {bad}");
            }
        }
    }
}
