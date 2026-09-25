//! A change, read back from `git diff` text into structure. Pure: no
//! filesystem, no process. Paths and bodies are somebody else's bytes, carried
//! verbatim; the interface escapes them by construction, so there is no HTML
//! and no escaper here.

use serde::{Deserialize, Serialize};

/// How much of a change is carried before the rest is reported, with the
/// command that shows it, rather than silently dropped.
pub const MAX_FILES: usize = 60;
pub const MAX_LINES: usize = 4_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Added,
    Modified,
    Deleted,
    Renamed { from: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Context,
    Added,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hunk {
    /// The `@@ … @@` line, verbatim.
    pub header: String,
    pub lines: Vec<(Kind, String)>,
}

/// A file's change, or why there is nothing to show (`Binary`) versus why it
/// was not shown (`Skipped`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Body {
    Hunks(Vec<Hunk>),
    Binary { bytes: Option<u64> },
    Skipped { why: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub status: Status,
    pub added: u32,
    pub removed: u32,
    pub body: Body,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Truncation {
    pub files_shown: usize,
    pub files_total: usize,
    /// The exact command that shows the whole thing.
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    /// The merge base, never the branch tip.
    pub base: String,
    pub files: Vec<FileChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<Truncation>,
}

impl ChangeSet {
    /// A branch that changed nothing is a finding, not an empty state: a gate
    /// that passed over no change verified nothing.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn totals(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

/// Reads `git diff` output into a change set, without interpreting it.
pub fn parse(base: &str, text: &str, command: &str) -> ChangeSet {
    let mut files: Vec<FileChange> = Vec::new();
    let mut total = 0usize;
    let mut lines_rendered = 0usize;
    let mut cur: Option<FileChange> = None;
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut hunk: Option<Hunk> = None;
    // Lines still due in the current hunk (old, new). Content is recognised by
    // count, never by shape: a removed `-- x` arrives as `--- x`.
    let mut left = (0u32, 0u32);

    let flush_hunk = |hunk: &mut Option<Hunk>, hunks: &mut Vec<Hunk>| {
        if let Some(h) = hunk.take() {
            hunks.push(h);
        }
    };

    for raw in text.lines() {
        if let Some(rest) = raw.strip_prefix("diff --git ") {
            flush_hunk(&mut hunk, &mut hunks);
            if let Some(mut f) = cur.take() {
                f.body = finish_body(f.body, std::mem::take(&mut hunks));
                push(&mut files, f, &mut total);
            }
            hunks = Vec::new();
            left = (0, 0);
            cur = Some(FileChange {
                path: path_of(rest),
                status: Status::Modified,
                added: 0,
                removed: 0,
                body: Body::Hunks(Vec::new()),
            });
            continue;
        }
        let Some(f) = cur.as_mut() else { continue };

        if raw.starts_with("new file mode") {
            f.status = Status::Added;
        } else if raw.starts_with("deleted file mode") {
            f.status = Status::Deleted;
        } else if let Some(from) = raw.strip_prefix("rename from ") {
            f.status = Status::Renamed {
                from: from.to_string(),
            };
        } else if raw.starts_with("Binary files ") || raw.starts_with("GIT binary patch") {
            f.body = Body::Binary { bytes: None };
        } else if (left.0 > 0 || left.1 > 0) && hunk.is_some() {
            let Some(h) = hunk.as_mut() else { continue };
            // `\ No newline at end of file` is neither side.
            if raw.starts_with('\\') {
                continue;
            }
            let (kind, body) = match raw.as_bytes().first() {
                Some(b'+') => (Kind::Added, &raw[1..]),
                Some(b'-') => (Kind::Removed, &raw[1..]),
                _ => (Kind::Context, raw.strip_prefix(' ').unwrap_or(raw)),
            };
            match kind {
                Kind::Added => {
                    f.added += 1;
                    left.1 = left.1.saturating_sub(1);
                }
                Kind::Removed => {
                    f.removed += 1;
                    left.0 = left.0.saturating_sub(1);
                }
                Kind::Context => {
                    left.0 = left.0.saturating_sub(1);
                    left.1 = left.1.saturating_sub(1);
                }
            }
            if lines_rendered < MAX_LINES {
                h.lines.push((kind, body.to_string()));
                lines_rendered += 1;
            } else if !matches!(f.body, Body::Skipped { .. }) {
                f.body = Body::Skipped {
                    why: "the change is longer than this view renders".into(),
                };
            }
        } else if raw.starts_with("@@") {
            flush_hunk(&mut hunk, &mut hunks);
            left = hunk_counts(raw);
            hunk = Some(Hunk {
                header: raw.to_string(),
                lines: Vec::new(),
            });
        }
        // Anything else between hunks — `index`, `--- a/…`, `+++ b/…`, mode
        // lines — is header, not content.
    }
    flush_hunk(&mut hunk, &mut hunks);
    if let Some(mut f) = cur.take() {
        f.body = finish_body(f.body, hunks);
        push(&mut files, f, &mut total);
    }

    let truncated = (total > files.len()).then(|| Truncation {
        files_shown: files.len(),
        files_total: total,
        command: command.to_string(),
    });
    ChangeSet {
        base: base.to_string(),
        files,
        truncated,
    }
}

/// The old and new line counts of an `@@ -a,b +c,d @@` header; omitted is one.
fn hunk_counts(header: &str) -> (u32, u32) {
    let mut words = header.split_whitespace().skip(1);
    let count = |w: Option<&str>, sign: char| -> u32 {
        let Some(w) = w.and_then(|w| w.strip_prefix(sign)) else {
            return 0;
        };
        match w.split_once(',') {
            Some((_, n)) => n.parse().unwrap_or(0),
            None => 1,
        }
    };
    let old = count(words.next(), '-');
    let new = count(words.next(), '+');
    (old, new)
}

/// Keeps a parsed body unless the file already declared itself unrenderable.
fn finish_body(body: Body, hunks: Vec<Hunk>) -> Body {
    match body {
        Body::Hunks(_) => Body::Hunks(hunks),
        other => other,
    }
}

fn push(files: &mut Vec<FileChange>, f: FileChange, total: &mut usize) {
    *total += 1;
    if files.len() < MAX_FILES {
        files.push(f);
    }
}

/// The `b/` side of a `diff --git a/x b/x` line: a rename's new name.
fn path_of(rest: &str) -> String {
    match rest.rsplit_once(" b/") {
        Some((_, b)) => b.to_string(),
        None => rest.trim_start_matches("a/").to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/api.rs b/src/api.rs
index 1111111..2222222 100644
--- a/src/api.rs
+++ b/src/api.rs
@@ -742,6 +742,7 @@ fn gate_view(
 fn gate_view(
-    old line
+    new line
+    another
diff --git a/notes.md b/readme.md
similarity index 90%
rename from notes.md
rename to readme.md
diff --git a/new.rs b/new.rs
new file mode 100644
--- /dev/null
+++ b/new.rs
@@ -0,0 +1,1 @@
+fn main() {}
diff --git a/gone.rs b/gone.rs
deleted file mode 100644
@@ -1,1 +0,0 @@
-fn main() {}
diff --git a/logo.png b/logo.png
Binary files a/logo.png and b/logo.png differ
";

    /// A removed SQL comment `-- x` arrives as `--- x`; an added `++ y` as
    /// `+++ y`. Both are content, because the header counted them.
    #[test]
    fn content_that_looks_like_a_file_header_is_content() {
        let text = "\
diff --git a/q.sql b/q.sql
index 1..2 100644
--- a/q.sql
+++ b/q.sql
@@ -1,2 +1,2 @@
--- the old comment
+++ the new one
 select 1;
";
        let set = parse("main", text, "git diff");
        let f = &set.files[0];
        assert_eq!((f.added, f.removed), (1, 1));
        let Body::Hunks(h) = &f.body else { panic!() };
        assert_eq!(h[0].lines[0], (Kind::Removed, "-- the old comment".into()));
        assert_eq!(h[0].lines[1], (Kind::Added, "++ the new one".into()));
        assert_eq!(h[0].lines.len(), 3);
    }

    #[test]
    fn every_status_a_change_can_have_is_read_back() {
        let set = parse("main", SAMPLE, "git diff main...HEAD");
        let by = |p: &str| {
            set.files
                .iter()
                .find(|f| f.path == p)
                .unwrap_or_else(|| panic!("no {p} in {:?}", set.files))
                .clone()
        };
        assert_eq!(by("src/api.rs").status, Status::Modified);
        assert_eq!(by("src/api.rs").added, 2);
        assert_eq!(by("src/api.rs").removed, 1);
        assert_eq!(by("new.rs").status, Status::Added);
        assert_eq!(by("gone.rs").status, Status::Deleted);
        assert_eq!(
            by("readme.md").status,
            Status::Renamed {
                from: "notes.md".into()
            },
            "a rename keeps the name the file now has, and says where it came from"
        );
        assert!(matches!(by("logo.png").body, Body::Binary { .. }));
        // The file header is not content.
        let h = match by("src/api.rs").body {
            Body::Hunks(h) => h,
            other => panic!("{other:?}"),
        };
        assert_eq!(h.len(), 1);
        assert!(h[0].header.starts_with("@@"));
        assert!(
            !h[0].lines.iter().any(|(_, l)| l.starts_with("-- a/")),
            "`--- a/…` is the header, not a removed line"
        );
    }

    #[test]
    fn a_change_cannot_carry_markup_onto_the_page() {
        let hostile = "\
diff --git a/<img src=x onerror=alert(1)>.rs b/<img src=x onerror=alert(1)>.rs
--- a/x
+++ b/x
@@ -1 +1 @@
-<script>alert('old')</script>
+<script>alert(\"new\")</script>
";
        // Carried verbatim: the interface escapes. A parser that sanitised
        // would change the diff a reviewer approves.
        let set = parse("main", hostile, "git diff");
        let f = &set.files[0];
        assert_eq!(
            f.path, "<img src=x onerror=alert(1)>.rs",
            "the path was altered"
        );
        let Body::Hunks(hunks) = &f.body else {
            panic!("expected hunks");
        };
        let lines: Vec<&str> = hunks[0].lines.iter().map(|(_, t)| t.as_str()).collect();
        assert!(
            lines
                .iter()
                .any(|l| l.contains("<script>alert('old')</script>")),
            "the removed line was not carried verbatim: {lines:?}"
        );
        assert!(
            lines
                .iter()
                .any(|l| l.contains(r#"<script>alert("new")</script>"#)),
            "the added line was not carried verbatim: {lines:?}"
        );
    }

    #[test]
    fn a_change_past_the_bound_says_so_rather_than_showing_a_subset() {
        let mut big = String::new();
        for i in 0..(MAX_FILES + 5) {
            big.push_str(&format!(
                "diff --git a/f{i}.rs b/f{i}.rs\n@@ -1 +1 @@\n+x\n"
            ));
        }
        let set = parse("main", &big, "git diff main...HEAD");
        let t = set
            .truncated
            .clone()
            .expect("past the bound, this is reported");
        assert_eq!(t.files_shown, MAX_FILES);
        assert_eq!(t.files_total, MAX_FILES + 5);
        assert!(t.command.contains("git diff"), "and how to see the rest");
    }

    #[test]
    fn a_branch_that_changed_nothing_says_that_in_words() {
        let set = parse("main", "", "git diff");
        assert!(set.is_empty());
        assert!(
            set.truncated.is_none(),
            "empty is not truncated, which is a different claim"
        );
    }

    #[test]
    fn the_totals_are_the_sum_of_the_files() {
        // `src/api.rs` +2/-1, `new.rs` +1, `gone.rs` -1; rename and binary add none.
        let set = parse("main", SAMPLE, "git diff");
        assert_eq!(set.totals(), (3, 2));
    }
}

#[cfg(test)]
mod sc003 {
    /// Parse time for a 500-modified-line change, on input it builds itself.
    /// Ignored: a timing assertion on a loaded machine is noise.
    #[test]
    #[ignore]
    fn measure() {
        let mut text = String::new();
        // 28 files × 18 changed (added plus removed) lines each.
        for f in 0..28 {
            text.push_str(&format!("diff --git a/src/mod{f}.rs b/src/mod{f}.rs\n"));
            text.push_str("--- a/x\n+++ b/x\n@@ -1,20 +1,22 @@\n");
            for i in 0..42 {
                let k = if i % 3 == 0 {
                    '+'
                } else if i % 7 == 0 {
                    '-'
                } else {
                    ' '
                };
                text.push_str(&format!(
                    "{k}    let value_{i} = compute(&state, {i}); // line {i}\n"
                ));
            }
        }

        let t = std::time::Instant::now();
        let set = super::parse("main", &text, "git diff main...HEAD");
        let parsed = t.elapsed();
        let (a, r) = set.totals();
        assert!(a + r >= 500, "the sample is the size the criterion names");
        println!(
            "diff budget: {} files, +{a}/-{r} lines · parse {parsed:?}",
            set.files.len()
        );
    }
}
