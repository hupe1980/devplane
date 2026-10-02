//! A person's comments on the lines of a change's review, and the one message
//! they are sent as. Pure: the host keeps the rows and delivers the message.
//!
//! Each comment carries what it was written against — the line's text, its
//! hunk's digest and the tree — so both the agent and the review can tell a
//! comment the code has since moved past. The code line is quoted in a fence:
//! it is somebody else's text, carried as data.

use crate::core::diff::{ChangeSet, Hunk, Kind};
use crate::core::review::{hunk_digest, hunks_of};
use serde::{Deserialize, Serialize};

/// One comment on one line, as the host keeps it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub struct ReviewComment {
    pub id: String,
    pub change_id: String,
    pub path: String,
    /// The line's number on `side`.
    pub line: u32,
    /// `new` for an added or context line, `old` for a removed one.
    pub side: String,
    pub hunk_header: String,
    pub hunk_digest: String,
    /// The line's text when the comment was written.
    pub line_text: String,
    pub body: String,
    /// The tree the comment was written against.
    pub tree: String,
    pub created_at: String,
    pub sent_at: Option<String>,
    /// The run it was sent to.
    pub run_id: Option<String>,
}

/// Where a comment's line is now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "is")]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
pub enum Anchor {
    /// The line is where it was, with the same text.
    Same,
    /// The same text, once, at another line of the file.
    Moved { line: u32 },
    /// The line is not in the change any more as it was written.
    Changed,
}

/// The lines of a hunk with their numbers: `(kind, text, old, new)`.
pub fn numbered(h: &Hunk) -> Vec<(Kind, &str, Option<u32>, Option<u32>)> {
    let (mut o, mut n) = starts(&h.header);
    h.lines
        .iter()
        .map(|(k, t)| match k {
            Kind::Added => {
                n += 1;
                (*k, t.as_str(), None, Some(n - 1))
            }
            Kind::Removed => {
                o += 1;
                (*k, t.as_str(), Some(o - 1), None)
            }
            Kind::Context => {
                o += 1;
                n += 1;
                (*k, t.as_str(), Some(o - 1), Some(n - 1))
            }
        })
        .collect()
}

/// The first old and new line numbers of `@@ -a,b +c,d @@`.
fn starts(header: &str) -> (u32, u32) {
    let num = |s: &str| {
        s.split(',')
            .next()
            .and_then(|x| x.parse::<u32>().ok())
            .unwrap_or(0)
    };
    let mut o = 0;
    let mut n = 0;
    for part in header.split_whitespace() {
        if let Some(x) = part.strip_prefix('-') {
            o = num(x);
        } else if let Some(x) = part.strip_prefix('+') {
            n = num(x);
            break;
        }
    }
    (o, n)
}

/// The hunk and the text at `(path, side, line)` in the change as it is, if
/// the diff shows that line.
pub fn locate<'a>(
    set: &'a ChangeSet,
    path: &str,
    side: &str,
    line: u32,
) -> Option<(&'a Hunk, String)> {
    let f = set.uncut().files.iter().find(|f| f.path == path)?;
    hunks_of(&f.body).iter().find_map(|h| {
        numbered(h)
            .into_iter()
            .find(|(_, _, o, n)| match side {
                "old" => *o == Some(line) && n.is_none(),
                _ => *n == Some(line),
            })
            .map(|(_, t, _, _)| (h, t.to_string()))
    })
}

/// Where the comment's line is in the change as it is now.
pub fn anchor(c: &ReviewComment, set: &ChangeSet) -> Anchor {
    if locate(set, &c.path, &c.side, c.line).is_some_and(|(_, t)| t == c.line_text) {
        return Anchor::Same;
    }
    let Some(f) = set.uncut().files.iter().find(|f| f.path == c.path) else {
        return Anchor::Changed;
    };
    let at: Vec<u32> = hunks_of(&f.body)
        .iter()
        .flat_map(numbered)
        .filter(|(k, t, _, _)| {
            *t == c.line_text
                && match c.side.as_str() {
                    "old" => *k == Kind::Removed,
                    _ => *k != Kind::Removed,
                }
        })
        .filter_map(|(_, _, o, n)| if c.side == "old" { o } else { n })
        .collect();
    match at.as_slice() {
        [line] => Anchor::Moved { line: *line },
        _ => Anchor::Changed,
    }
}

/// Whether the hunk a comment was written against is still in the change.
pub fn hunk_still_there(c: &ReviewComment, set: &ChangeSet) -> bool {
    set.uncut()
        .files
        .iter()
        .filter(|f| f.path == c.path)
        .flat_map(|f| hunks_of(&f.body).iter())
        .any(|h| hunk_digest(h) == c.hunk_digest)
}

/// The one message every unsent comment goes as, in the order given (the
/// review's): a heading with the count, then one block per comment naming
/// the path and line, what it was written against, the line as quoted data,
/// and the person's words.
pub fn compose(comments: &[&ReviewComment]) -> String {
    let mut out = format!("## Review comments ({})\n", comments.len());
    for c in comments {
        let at = match c.side.as_str() {
            "old" => format!("base line {}", c.line),
            _ => format!("line {}", c.line),
        };
        out.push_str(&format!(
            "\n**{}** ({at})\nwritten against tree `{}`, hunk `{}` (`{}`)\n",
            c.path, c.tree, c.hunk_digest, c.hunk_header
        ));
        let fence = fence_for(&c.line_text);
        out.push_str(&format!("{fence}\n{}\n{fence}\n", c.line_text));
        for l in c.body.lines() {
            out.push_str("> ");
            out.push_str(l);
            out.push('\n');
        }
    }
    out
}

/// A fence longer than any run of backticks in the text, so quoted code
/// cannot close it.
fn fence_for(text: &str) -> String {
    let mut longest = 0;
    let mut run = 0;
    for ch in text.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::diff::{Body, FileChange, Status};

    fn hunk(header: &str, lines: &[(Kind, &str)]) -> Hunk {
        Hunk {
            header: header.into(),
            lines: lines.iter().map(|(k, t)| (*k, t.to_string())).collect(),
        }
    }

    fn set(files: Vec<(&str, Vec<Hunk>)>) -> ChangeSet {
        ChangeSet {
            base: "main".into(),
            files: files
                .into_iter()
                .map(|(p, h)| FileChange {
                    path: p.into(),
                    status: Status::Modified,
                    added: 1,
                    removed: 0,
                    body: Body::Hunks(h),
                })
                .collect(),
            ..ChangeSet::default()
        }
    }

    fn comment(path: &str, line: u32, side: &str, text: &str, body: &str) -> ReviewComment {
        ReviewComment {
            id: format!("rc-{path}-{line}"),
            change_id: "c-1".into(),
            path: path.into(),
            line,
            side: side.into(),
            hunk_header: "@@ -1,1 +1,2 @@".into(),
            hunk_digest: "00000000000000aa".into(),
            line_text: text.into(),
            body: body.into(),
            tree: "t1".into(),
            created_at: "2026-10-02T10:00:00.000000000Z".into(),
            sent_at: None,
            run_id: None,
        }
    }

    #[test]
    fn three_comments_on_two_files_are_one_message_of_blocks_in_order() {
        let a = comment("src/a.rs", 2, "new", "let x = 1;", "why 1?");
        let b = comment("src/a.rs", 7, "old", "check();", "keep this\nplease");
        let c = comment("src/b.rs", 3, "new", "it.skip(`x`)", "unskip");
        let m = compose(&[&a, &b, &c]);
        assert!(m.starts_with("## Review comments (3)\n"), "{m}");
        assert_eq!(m.matches("## Review comments").count(), 1);
        let ia = m.find("**src/a.rs** (line 2)").unwrap();
        let ib = m.find("**src/a.rs** (base line 7)").unwrap();
        let ic = m.find("**src/b.rs** (line 3)").unwrap();
        assert!(ia < ib && ib < ic, "{m}");
        assert!(m.contains("written against tree `t1`, hunk `00000000000000aa`"));
        assert!(m.contains("```\nlet x = 1;\n```"), "{m}");
        assert!(m.contains("> keep this\n> please\n"), "{m}");
    }

    #[test]
    fn quoted_code_cannot_close_its_fence() {
        let c = comment("a.md", 1, "new", "```", "x");
        let m = compose(&[&c]);
        assert!(m.contains("````\n```\n````"), "{m}");
    }

    #[test]
    fn an_anchor_is_same_moved_or_changed() {
        let s = set(vec![(
            "a.rs",
            vec![hunk(
                "@@ -1,2 +1,3 @@",
                &[
                    (Kind::Context, "fn a() {}"),
                    (Kind::Added, "let x = 1;"),
                    (Kind::Context, "fn b() {}"),
                ],
            )],
        )]);
        assert_eq!(
            anchor(&comment("a.rs", 2, "new", "let x = 1;", "?"), &s),
            Anchor::Same
        );
        assert_eq!(
            anchor(&comment("a.rs", 9, "new", "let x = 1;", "?"), &s),
            Anchor::Moved { line: 2 }
        );
        assert_eq!(
            anchor(&comment("a.rs", 2, "new", "let x = 2;", "?"), &s),
            Anchor::Changed
        );
        assert_eq!(
            anchor(&comment("gone.rs", 2, "new", "let x = 1;", "?"), &s),
            Anchor::Changed
        );
    }

    #[test]
    fn a_removed_line_is_found_on_the_base_side() {
        let s = set(vec![(
            "a.rs",
            vec![hunk(
                "@@ -4,2 +4,1 @@",
                &[(Kind::Removed, "old();"), (Kind::Context, "kept();")],
            )],
        )]);
        let (_, t) = locate(&s, "a.rs", "old", 4).unwrap();
        assert_eq!(t, "old();");
        assert!(locate(&s, "a.rs", "new", 4).is_some_and(|(_, t)| t == "kept();"));
        assert!(locate(&s, "a.rs", "old", 9).is_none());
    }

    #[test]
    fn a_comment_knows_whether_its_hunk_is_still_there() {
        let h = hunk("@@ -1 +1 @@", &[(Kind::Added, "x")]);
        let s = set(vec![("a.rs", vec![h.clone()])]);
        let mut c = comment("a.rs", 1, "new", "x", "?");
        c.hunk_digest = hunk_digest(&h);
        assert!(hunk_still_there(&c, &s));
        let s2 = set(vec![(
            "a.rs",
            vec![hunk("@@ -1 +1 @@", &[(Kind::Added, "y")])],
        )]);
        assert!(!hunk_still_there(&c, &s2));
    }
}
