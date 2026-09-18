//! A change, read back and rendered.
//!
//! **Pure on purpose.** Text in, structure out, HTML out — no filesystem, no
//! process, no waiting. `tests/purity.rs` holds everything under `src/core/` to
//! that, which is the whole reason this lives here rather than beside the git
//! call that produces its input: a renderer asserted to be pure with nothing
//! enforcing it is a claim, and this project's own specification process caught
//! exactly that before a line was written.
//!
//! **Everything here is somebody else's bytes.** A diff body is repository
//! contents and a path is whatever an agent named a file; both are escaped on
//! the way out, once, in one place, so no caller can forget.
//!
//! **And the daemon renders rather than the browser.** The board ships no build
//! step and no bundler, and a client-side diff renderer is the change that
//! would force one. The daemon is on loopback, so a round trip costs a
//! millisecond and the rendering is covered by this crate's tests instead of by
//! nothing.

use serde::{Deserialize, Serialize};

/// How much of a change is rendered before the rest is reported rather than
/// shown.
///
/// A bound rather than a guess at what is readable: a work that touched
/// hundreds of files must still open promptly, and the honest answer past the
/// bound is *here is what is not shown and the command that shows it* — never a
/// silent subset.
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
    /// The `@@ … @@` line, verbatim — it locates the change in the file.
    pub header: String,
    pub lines: Vec<(Kind, String)>,
}

/// What a file's change looks like, or why it does not look like anything.
///
/// Three variants because *there is nothing to show* and *we chose not to show
/// it* are different sentences, and a reviewer deciding whether to approve is
/// entitled to know which one they are reading.
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
    /// The exact command that shows the whole thing. A reviewer told only that
    /// something is missing has been told half of what they need.
    pub command: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChangeSet {
    /// What was compared against — the merge base, never the branch tip.
    pub base: String,
    pub files: Vec<FileChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub truncated: Option<Truncation>,
}

impl ChangeSet {
    /// Whether the branch changed nothing.
    ///
    /// A **finding**, not an empty state: a gate that passed over no change
    /// verified nothing, and the view says so rather than showing a blank.
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    pub fn totals(&self) -> (u32, u32) {
        self.files
            .iter()
            .fold((0, 0), |(a, r), f| (a + f.added, r + f.removed))
    }
}

/// Reads `git diff` output into a change set.
///
/// Hand-written rather than a crate, for the reason the rest of this project
/// gives: the format is small, closed and stable, and the alternative is a
/// dependency for one function. What it does **not** do is interpret — no
/// language, no syntax, no guess about what a change means.
pub fn parse(base: &str, text: &str, command: &str) -> ChangeSet {
    let mut files: Vec<FileChange> = Vec::new();
    let mut total = 0usize;
    let mut lines_rendered = 0usize;
    let mut cur: Option<FileChange> = None;
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut hunk: Option<Hunk> = None;

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
        } else if raw.starts_with("@@") {
            flush_hunk(&mut hunk, &mut hunks);
            hunk = Some(Hunk {
                header: raw.to_string(),
                lines: Vec::new(),
            });
        } else if let Some(h) = hunk.as_mut() {
            // `--- a/…` and `+++ b/…` are the file header, not content, and a
            // `\ No newline at end of file` marker is neither.
            if raw.starts_with("--- ") || raw.starts_with("+++ ") || raw.starts_with('\\') {
                continue;
            }
            let (kind, body) = match raw.as_bytes().first() {
                Some(b'+') => (Kind::Added, &raw[1..]),
                Some(b'-') => (Kind::Removed, &raw[1..]),
                _ => (Kind::Context, raw.strip_prefix(' ').unwrap_or(raw)),
            };
            match kind {
                Kind::Added => f.added += 1,
                Kind::Removed => f.removed += 1,
                Kind::Context => {}
            }
            if lines_rendered < MAX_LINES {
                h.lines.push((kind, body.to_string()));
                lines_rendered += 1;
            } else if !matches!(f.body, Body::Skipped { .. }) {
                f.body = Body::Skipped {
                    why: "the change is longer than this view renders".into(),
                };
            }
        }
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

/// The path out of a `diff --git a/x b/x` line.
fn path_of(rest: &str) -> String {
    // The `b/` side, because for a rename it is the name the file now has.
    match rest.rsplit_once(" b/") {
        Some((_, b)) => b.to_string(),
        None => rest.trim_start_matches("a/").to_string(),
    }
}

/// HTML-escapes one value. The only way text leaves this module.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The change as HTML the page inserts.
///
/// Line classes and nothing else — no syntax highlighting, no language
/// detection, no collapsing. The page supplies the colours.
pub fn render(set: &ChangeSet) -> String {
    if set.is_empty() {
        return "<div class=\"dnone\">This branch changed nothing. \
                A gate that passed over no change has verified nothing.</div>"
            .to_string();
    }
    let mut out = String::new();
    for f in &set.files {
        let (verb, extra) = match &f.status {
            Status::Added => ("added", String::new()),
            Status::Deleted => ("deleted", String::new()),
            Status::Modified => ("modified", String::new()),
            Status::Renamed { from } => ("renamed", format!(" from {}", esc(from))),
        };
        out.push_str(&format!(
            "<div class=\"dfile\"><span class=\"dpath\">{}</span>\
             <span class=\"dstat\">{verb}{extra} · +{} −{}</span></div>",
            esc(&f.path),
            f.added,
            f.removed
        ));
        match &f.body {
            Body::Binary { bytes } => out.push_str(&format!(
                "<div class=\"dnote\">binary{}</div>",
                bytes.map(|b| format!(", {b} bytes")).unwrap_or_default()
            )),
            Body::Skipped { why } => {
                out.push_str(&format!("<div class=\"dnote\">{}</div>", esc(why)))
            }
            Body::Hunks(hunks) => {
                for h in hunks {
                    out.push_str(&format!("<div class=\"dhunk\">{}</div>", esc(&h.header)));
                    out.push_str("<pre class=\"dbody\">");
                    for (kind, line) in &h.lines {
                        let (cls, sign) = match kind {
                            Kind::Added => ("da", "+"),
                            Kind::Removed => ("dr", "-"),
                            Kind::Context => ("dc", " "),
                        };
                        out.push_str(&format!(
                            "<span class=\"{cls}\">{sign}{}</span>\n",
                            esc(line)
                        ));
                    }
                    out.push_str("</pre>");
                }
            }
        }
    }
    if let Some(t) = &set.truncated {
        out.push_str(&format!(
            "<div class=\"dnote\">{} of {} files shown. The rest: <code>{}</code></div>",
            t.files_shown,
            t.files_total,
            esc(&t.command)
        ));
    }
    out
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
        // Nothing to show is not the same as chose not to show.
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

    /// Every byte that reaches the page is escaped.
    ///
    /// A diff body is repository contents and a path is whatever an agent named
    /// a file. Both are the reason this module has exactly one way out.
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
        let html = render(&parse("main", hostile, "git diff"));
        assert!(!html.contains("<script>"), "{html}");
        assert!(!html.contains("<img src=x"), "{html}");
        assert!(html.contains("&lt;script&gt;"));
        assert!(html.contains("&quot;new&quot;") || html.contains("&#39;old&#39;"));
    }

    /// A change too large reports what is missing, and how to see it.
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
        let html = render(&set);
        assert!(html.contains("of"), "the count is on the page: {html}");
        assert!(html.contains("git diff"));
    }

    /// No change at all is a finding, not a blank.
    #[test]
    fn a_branch_that_changed_nothing_says_that_in_words() {
        let set = parse("main", "", "git diff");
        assert!(set.is_empty());
        assert!(set.truncated.is_none());
        let html = render(&set);
        assert!(
            html.contains("has verified nothing"),
            "a gate that passed over no change is worth a sentence: {html}"
        );
    }

    #[test]
    fn the_totals_are_the_sum_of_the_files() {
        // `src/api.rs` is +2/-1, `new.rs` is +1, `gone.rs` is -1. The rename
        // carries no hunk and the binary file carries no lines, so neither
        // contributes — which is the point of counting from the parsed files
        // rather than from the raw text.
        let set = parse("main", SAMPLE, "git diff");
        assert_eq!(set.totals(), (3, 2));
    }
}

/// Every edge case gets its own sentence.
///
/// The failure this guards against is not a crash — it is four different
/// situations rendering as the same blank region, so a reviewer reads *nothing
/// changed* where the truth was *we chose not to show you*. Walked once by hand
/// against a real repository; this is the part of that walk a machine can
/// repeat.
#[cfg(test)]
mod sc004 {
    use super::*;

    fn one(body: Body) -> String {
        render(&ChangeSet {
            base: "main".into(),
            files: vec![FileChange {
                path: "asset.bin".into(),
                status: Status::Modified,
                added: 0,
                removed: 0,
                body,
            }],
            truncated: None,
        })
    }

    #[test]
    fn each_edge_case_reads_differently() {
        let empty = render(&ChangeSet {
            base: "main".into(),
            files: Vec::new(),
            truncated: None,
        });
        let binary = one(Body::Binary { bytes: None });
        let sized = one(Body::Binary {
            bytes: Some(4_194_304),
        });
        let skipped = one(Body::Skipped {
            why: "too large to render".into(),
        });
        let mut truncated = ChangeSet {
            base: "main".into(),
            files: vec![FileChange {
                path: "asset.bin".into(),
                status: Status::Modified,
                added: 0,
                removed: 0,
                body: Body::Binary { bytes: None },
            }],
            truncated: Some(Truncation {
                files_shown: 60,
                files_total: 312,
                command: "git diff main...HEAD".into(),
            }),
        };
        let more = render(&truncated);
        truncated.truncated = None;

        let all = [&empty, &binary, &sized, &skipped, &more];
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert_ne!(a, b, "two edge cases render the same");
            }
        }
        // And each says the thing it is about, not merely something different.
        assert!(empty.contains("verified nothing"));
        assert!(sized.contains("4194304 bytes"), "{sized}");
        assert!(skipped.contains("too large to render"));
        assert!(more.contains("60 of 312") && more.contains("git diff main...HEAD"));
    }
}

#[cfg(test)]
mod sc003 {
    /// The render budget, measured rather than asserted — and reproducible.
    ///
    /// The specification says the view opens in under two seconds for a change
    /// of up to 500 modified lines. That bound covers the whole round trip —
    /// git, parse, render, insert — and only the middle two happen here, so
    /// what this records is the part measurable in this crate.
    ///
    /// **It builds its own input.** A measurement that depends on a file in
    /// `/tmp` that nothing creates is a number nobody can check, which is the
    /// opposite of the point. `--ignored`, because a timing assertion on a
    /// loaded machine fails for reasons that have nothing to do with the code —
    /// the number belongs in the specification's quickstart, taken by hand.
    #[test]
    #[ignore]
    fn measure() {
        let mut text = String::new();
        // 28 files × 18 changed lines each. Sized to the criterion's own unit:
        // *modified* lines means added plus removed, not lines of diff — the
        // first sample had 504 lines of diff and only 216 changes, and the
        // assertion below is what said so.
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
        let t = std::time::Instant::now();
        let html = super::render(&set);
        let rendered = t.elapsed();

        let (a, r) = set.totals();
        assert!(a + r >= 500, "the sample is the size the criterion names");
        println!(
            "SC-003: {} files, +{a}/-{r} lines · parse {parsed:?} · render {rendered:?} · {} KB of HTML",
            set.files.len(),
            html.len() / 1024
        );
    }
}
