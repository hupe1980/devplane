//! The done certificate: evidence a reviewer can check without trusting this
//! tool. It borrows the in-toto statement shape but is deliberately unsigned:
//! SLSA provenance asks the reader to trust the builder, whereas this names a
//! commit and the gate commands so the reader re-runs them. It establishes only
//! that these commands ended as recorded against this commit — not that the
//! work is correct — and says so in its own text ([`LIMITS`]).

use super::change::{Change, CommandResult, CommitStamp, Completion, GateReport, Outcome, Reach};

/// Where a piece of evidence came from, in the OpenTelemetry GenAI conventions'
/// proposed vocabulary (not yet a standard). An unknown origin is absent, never
/// defaulted — the default would be the flattering one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A gate transcript: commands this tool ran and their exit codes;
    /// re-derivable by a reviewer.
    ExternallyObserved,
    /// The agent's own account of what it did: a claim, never a predicate.
    SelfReported,
}

impl Origin {
    pub const KEY: &'static str = "gen_ai.evidence.origin";

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Origin::ExternallyObserved => "externally_observed",
            Origin::SelfReported => "self_reported",
        }
    }
}

/// The predicate type this emits, versioned.
pub const PREDICATE_TYPE: &str = "https://devplane.dev/DoneCertificate/v1";
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";

/// The artifact's hard size ceiling.
pub const MAX_BYTES: usize = 64 * 1024;

/// How much of a command a page row shows; the copy carries the whole thing.
pub const PAGE_COMMAND_CHARS: usize = 120;

/// What the artifact claims, in its own words, so it travels with the paste.
pub const LIMITS: &str = "This is evidence that these commands ended as recorded against this \
commit. It is not evidence that the work is correct, that the commands check the right things, or \
that the specification was satisfied. It is unsigned and detects change rather than forgery.";

/// Every field a reader can check for themselves — one list shared by both renderings.
pub const REDERIVABLE: &[&str] = &[
    "evidence.commit.commit",
    "evidence.commit.tree",
    "evidence.commands[].command",
    "evidence.commands[].outcome",
    "specification.fingerprint",
    "specification.tasks_done",
    "specification.tasks_total",
];

/// The portable record of one change's completion. Assembled from a `Change`,
/// never stored.
pub struct Certificate<'a> {
    pub change: &'a Change,
    /// The gate run the basis points at, when it points at one.
    pub evidence: Option<&'a GateReport>,
    pub attempts: u32,
    /// The agent's own account, when the rules allow it to be shown at all.
    pub claim: Option<&'a str>,
    /// The checks the change itself altered, read from its diff. `None` when
    /// the worktree is gone — said, never rendered as *nothing altered*.
    pub weakened: Option<Vec<super::review::Weakened>>,
}

impl<'a> Certificate<'a> {
    /// Builds the certificate for a change. The evidence is the run the basis
    /// names, not the latest; if that run is gone the evidence is absent (no
    /// fallback to another run). With no basis yet, the latest `check` report.
    pub fn of(change: &'a Change, claim: Option<&'a str>) -> Self {
        let evidence = match change.completion.as_ref().and_then(Completion::gate) {
            Some((gate, attempt)) => change
                .gates
                .iter()
                .find(|g| g.gate == gate && g.attempt == attempt),
            None => change.check_report(),
        };
        Self {
            change,
            evidence,
            attempts: change.gates.len() as u32,
            claim,
            weakened: None,
        }
    }

    /// Whether the basis names a gate run the record no longer holds.
    pub fn basis_run_missing(&self) -> bool {
        self.evidence.is_none()
            && self
                .change
                .completion
                .as_ref()
                .and_then(Completion::gate)
                .is_some()
    }

    /// The sentence saying why there is no evidence, when there is none.
    fn no_evidence(&self) -> String {
        match self.change.completion.as_ref().and_then(Completion::gate) {
            Some((gate, attempt)) => format!(
                "The basis names `{gate}` attempt {attempt}, and that run is no longer in the \
                 record, so there is no evidence to show."
            ),
            None => "No gate ran, so there is no evidence to show.".into(),
        }
    }

    /// The sentence about the checks the change itself altered.
    pub fn weakened_says(&self) -> String {
        match &self.weakened {
            None => "Whether this change altered its own checks could not be read: its worktree \
                     is gone."
                .into(),
            Some(w) => super::review::weakened_sentence(w).unwrap_or_else(|| {
                "This change altered none of its checks: no skip marker added, no test file \
                 deleted, no gate or CI definition edited."
                    .into()
            }),
        }
    }

    /// A digest of the evidence's gate commands, in order.
    pub fn commands_digest(&self) -> Option<String> {
        let report = self.evidence?;
        let mut h = super::hash::Rolling::new();
        for c in &report.commands {
            h.push_str(&c.command);
        }
        Some(h.hex())
    }

    /// Whether this change is finished. Unfinished is a rendering branch, not an error.
    pub fn is_finished(&self) -> bool {
        self.change.completion.is_some()
    }

    /// The commands a reader runs to check this, in order.
    pub fn verification_steps(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Some(report) = self.evidence else {
            return out;
        };
        // The clone line comes first — a commit with no repository is an
        // instruction nobody can follow — and only when the stamp says the
        // commit is checkable by others.
        if let Some(url) = report
            .commit
            .as_ref()
            .filter(|c| c.checkable_by_others())
            .and_then(|c| c.remote.as_deref())
        {
            let q = shell_quote(url);
            out.push(format!("git clone {q} && cd \"$(basename {q} .git)\""));
        }
        if let Some(sha) = report.commit.as_ref().and_then(|c| c.commit.as_deref()) {
            out.push(format!("git checkout {sha}"));
        }
        out.extend(report.commands.iter().map(|c| c.command.clone()));
        out
    }

    /// Why a reader may not be able to follow those steps, if they cannot.
    pub fn verification_caveat(&self) -> Option<String> {
        let commit = self.evidence.and_then(|r| r.commit.as_ref())?;
        match (&commit.commit, &commit.reach) {
            (None, _) => {
                Some("This repository had no commits yet, so there is nothing to check out.".into())
            }
            (Some(_), Reach::LocalOnly) => Some(
                "This commit exists only on the machine that produced this certificate. \
                 It is on no remote, so you cannot fetch it and these steps will not work \
                 for you until it is pushed."
                    .into(),
            ),
            (Some(_), Reach::NoRemote) => Some(
                "This repository has no remote configured, so this commit is reachable only \
                 where it was made."
                    .into(),
            ),
            (Some(_), Reach::Unknown) => Some(
                "Whether this commit has been pushed could not be determined — not that it \
                 has not been."
                    .into(),
            ),
            (Some(_), Reach::Remote) => None,
        }
    }
}

/// One word for a POSIX shell. A remote URL is untrusted, and a pasted step
/// must not run anything it smuggled in.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// How a command's outcome reads — one function for both renderings.
pub fn outcome_sentence(c: &CommandResult) -> String {
    match &c.outcome {
        Outcome::Exited { code } if *code == 0 => "exit 0".to_string(),
        Outcome::Exited { code } => format!("exit {code}"),
        Outcome::TimedOut { after_secs } => {
            format!("no exit code — killed after {after_secs}s")
        }
        Outcome::NeverStarted { reason } => format!("never started: {reason}"),
        Outcome::Unknown { reason } => {
            format!("could not be determined: {reason} — this is not a result")
        }
    }
}

/// How the tree reads, where the commit is shown rather than in a footnote.
pub fn commit_sentence(c: &CommitStamp) -> String {
    // The tree is what the commands ran against; the commit is how a reviewer
    // gets to it.
    let head = match (&c.commit, &c.tree) {
        (Some(sha), Some(tree)) => format!("Commit `{sha}`, working-tree digest `{tree}`"),
        (Some(sha), None) => format!(
            "Commit `{sha}`, working-tree digest not recorded — the tree moved while the gate \
             ran, or could not be read, so this run cannot verify anything"
        ),
        (None, Some(tree)) => {
            format!("No commit yet — this repository had none; working-tree digest `{tree}`")
        }
        (None, None) => "No commit — this repository had no commits yet".to_string(),
    };
    let branch = match &c.branch {
        Some(b) => format!(" on branch `{b}`"),
        None => " on a detached head".to_string(),
    };
    let files = match c.clean {
        true => "clean — this commit is what was checked".to_string(),
        false => format!(
            "**{} uncommitted or untracked file(s)** — the commit alone does NOT describe what \
             was checked; the digest does",
            c.changed_files
        ),
    };
    let digest = match &c.tree {
        Some(_) => "\nThe digest is git's tree of the files as they were, untracked ones included \
                    and **ignored files excluded**; `git add -A && git write-tree` in a copy of \
                    that tree re-derives it."
            .to_string(),
        None => String::new(),
    };
    let tree = format!("Working tree: {files}.{digest}");
    let origin = match &c.remote {
        Some(url) => format!("Repository: `{}`\n", url.replace(['\n', '\r'], " ")),
        None => String::new(),
    };
    let reach = match c.reach {
        Reach::Remote => "This commit is on a remote, so you can fetch it.",
        Reach::LocalOnly => {
            "**This commit is on no remote.** Nobody but the machine that produced this can check it."
        }
        Reach::NoRemote => "This repository has no remote configured.",
        Reach::Unknown => "Whether this commit is on a remote could not be determined.",
    };
    format!("{origin}{head}{branch}\n{tree}\n{reach}")
}

/// A fence one backtick longer than the longest run inside the content (min
/// three), so untrusted commands, titles or output cannot close it and inject
/// Markdown.
fn fenced(content: &str) -> String {
    let longest = content
        .as_bytes()
        .split(|b| *b != b'`')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(longest.max(2) + 1);
    format!("{fence}\n{content}\n{fence}")
}

/// Untrusted text on one line of a table or sentence: pipes and newlines neutralised.
fn inline(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// Untrusted text inside a code span, where the delimiter has to clear it.
fn code(s: &str) -> String {
    let flat = s.replace(['\n', '\r'], " ");
    let longest = flat
        .as_bytes()
        .split(|b| *b != b'`')
        .map(<[u8]>::len)
        .max()
        .unwrap_or(0);
    let t = "`".repeat(longest + 1);
    let body = flat.replace('|', "\\|");
    // Pad only when a backtick at either end would fuse with the delimiter.
    match body.starts_with('`') || body.ends_with('`') {
        true => format!("{t} {body} {t}"),
        false => format!("{t}{body}{t}"),
    }
}

fn bytes_phrase(n: u64) -> String {
    match n {
        0 => "no output".into(),
        1 => "1 byte".into(),
        _ => format!("{n} bytes"),
    }
}

impl Certificate<'_> {
    /// The document a reviewer reads, sized for a pull request body.
    pub fn markdown(&self) -> String {
        let mut o = String::new();
        let w = self.change;

        o.push_str(&format!("# Done certificate — {}\n\n", inline(&w.title)));

        let Some(basis) = w.completion.as_ref() else {
            // Unfinished: say where it is — not an error, not a certificate.
            o.push_str(&format!(
                "**This change is not finished.** It is {}.\n\n",
                where_it_is(w)
            ));
            if let Some(g) = self.evidence {
                o.push_str(&format!("Its last gate said: {}\n\n", inline(&g.summary())));
            } else {
                o.push_str("No gate has run.\n\n");
            }
            o.push_str(&format!("{}\n\n", inline(&self.weakened_says())));
            o.push_str("There is no completion to certify yet.\n");
            return o;
        };

        o.push_str(&format!("**Basis:** {}\n\n", basis.headline()));
        if let Some(note) = basis.unchecked_note() {
            o.push_str(&format!("> {note}\n\n"));
        }

        o.push_str("## What was checked\n\n");
        match self.evidence {
            None => o.push_str(&format!("{}\n\n", self.no_evidence())),
            Some(report) => {
                // Name the origin: a gate transcript is observed by something
                // the agent does not control; the claim below carries the other value.
                o.push_str(&format!(
                    "`{}: {}`\n\n",
                    Origin::KEY,
                    Origin::ExternallyObserved.as_str()
                ));
                match report.commit.as_ref() {
                    Some(c) => o.push_str(&format!("{}\n\n", commit_sentence(c))),
                    None => o.push_str(
                        "No commit was recorded — this gate did not run in a repository.\n\n",
                    ),
                }
                o.push_str("| Command | Outcome | Took | Output |\n|---|---|---|---|\n");
                for c in &report.commands {
                    o.push_str(&format!(
                        "| {} | {} | {:.1}s | {} · `{}` |\n",
                        code(&c.command),
                        inline(&outcome_sentence(c)),
                        c.duration_ms as f64 / 1000.0,
                        bytes_phrase(c.output_bytes),
                        c.output_digest,
                    ));
                }
                o.push('\n');
                if let Some(d) = self.commands_digest() {
                    o.push_str(&format!("Gate commands digest: `{d}`.\n\n"));
                }
                if report.commands.iter().any(|c| c.output_bytes > 0) {
                    o.push_str(
                        "Output shown anywhere below is a **tail**; the byte counts above say how \
                         much there was.\n\n",
                    );
                }
            }
        }

        o.push_str(&format!("{}\n\n", inline(&self.weakened_says())));

        let steps = self.verification_steps();
        if !steps.is_empty() {
            o.push_str("## Check this yourself\n\n");
            if let Some(caveat) = self.verification_caveat() {
                o.push_str(&format!("> {caveat}\n\n"));
            }
            o.push_str(&fenced(&steps.join("\n")));
            o.push_str(
                "\n\nYou should see the same outcomes. You will probably **not** see the \
                        same output digests — see *What this does not establish*.\n\n",
            );
        }

        if let Some(report) = self.evidence
            && let Some(spec) = report.spec.as_ref()
        {
            o.push_str("## The specification it names\n\n");
            match &spec.fingerprint {
                Some(f) => o.push_str(&format!(
                    "{} · {} document(s) · fingerprint `{}`\n\n",
                    code(&spec.path),
                    spec.files,
                    f
                )),
                None => o.push_str(&format!(
                    "The change says it answers {} and there was no such file or folder.\n\n",
                    code(&spec.path)
                )),
            }
            // The plan moved under the change: a reviewer re-deriving the verdict
            // against today's document would get a different answer.
            if self.change.plan_drifted(spec.fingerprint.as_deref()) == Some(true) {
                o.push_str(&format!(
                    "> **The specification changed while this change was running.** It was `{}` when \
                     the change started and `{}` when the gate ran. The agent read one document and \
                     you are reading another; both may be correct.\n\n",
                    self.change.spec_at_start.as_deref().unwrap_or("unknown"),
                    spec.fingerprint.as_deref().unwrap_or("unknown"),
                ));
            }
            match spec.tasks() {
                Some((done, total)) => {
                    o.push_str(&format!("Task list: **{done} of {total} ticked**"));
                    if spec.open_questions > 0 {
                        o.push_str(&format!(", {} open question(s)", spec.open_questions));
                    }
                    o.push_str(".\n\n");
                    if spec.has_unticked_work() {
                        o.push_str(
                            "> Tasks were unticked when this ran. That is not a contradiction the \
                             gate can resolve and this tool does not resolve it either. Both \
                             numbers are here; you decide.\n\n",
                        );
                    }
                }
                None => o.push_str(
                    "Task list: none — the specification has no task list, which \
                                    is different from having made no progress.\n\n",
                ),
            }
        }

        if let Some(claim) = self.claim {
            o.push_str("## The agent's claim\n\n");
            // The same origin marker the statement carries.
            o.push_str(&format!(
                "`{}: {}`\n\n",
                Origin::KEY,
                Origin::SelfReported.as_str()
            ));
            o.push_str(
                "This is the agent's own account, shown beside the outcomes above and never in \
                 place of them. Across 5,851 sessions and 355,942 tool calls, such reports \
                 referenced about one action in eleven and drifted toward the plan as execution \
                 left it. Nothing here grades it against the evidence.\n\n",
            );
            o.push_str(&fenced(claim));
            o.push_str("\n\n");
        }

        o.push_str("## What this does not establish\n\n");
        o.push_str(LIMITS);
        o.push_str("\n\nFields you can re-derive yourself:\n\n");
        for f in REDERIVABLE {
            o.push_str(&format!("- `{f}`\n"));
        }
        o.push_str(
            "\nEverything else — timings, output digests, attempt counts, and anything this tool \
             recorded about its own history — you cannot. The digests **bind the stored output to \
             the run**; they are not reproducible, because stdout and stderr are captured \
             concurrently and interleave differently each time.\n",
        );
        o
    }

    /// The structured shape, as an in-toto statement.
    pub fn json(&self) -> serde_json::Value {
        use serde_json::json;
        let w = self.change;
        let commit = self.evidence.and_then(|r| r.commit.as_ref());
        // `subject` is what the statement is about: the commit.
        let subject = match commit.and_then(|c| c.commit.as_deref()) {
            Some(sha) => json!([{ "name": w.title, "digest": { "gitCommit": sha } }]),
            None => json!([{ "name": w.title, "digest": {} }]),
        };
        json!({
            "_type": STATEMENT_TYPE,
            "subject": subject,
            "predicateType": PREDICATE_TYPE,
            "predicate": {
                "change": { "id": w.id, "title": w.title, "state": w.current_state().as_str(),
                          "branch": w.branch },
                "completion": w.completion,
                "evidence": self.evidence.map(|r| json!({
                    // Observed by something the agent does not control.
                    Origin::KEY: Origin::ExternallyObserved.as_str(),
                    "gate": r.gate,
                    "at": r.at,
                    "attempt": r.attempt,
                    "attempts": self.attempts,
                    "passed": r.passed(),
                    "commit": r.commit,
                    "commands": r.commands,
                    "commands_digest": self.commands_digest(),
                })),
                "basis_run_missing": self.basis_run_missing(),
                "checks_altered": self.weakened,
                "specification": self.evidence.and_then(|r| r.spec.clone()),
                // An object so the origin rides with the text; absent entirely
                // when the agent said nothing.
                "agent_claim": self.claim.map(|text| json!({
                    Origin::KEY: Origin::SelfReported.as_str(),
                    "text": text,
                })),
                "verification": {
                    "steps": self.verification_steps(),
                    "caveat": self.verification_caveat(),
                },
                "rederivable": REDERIVABLE,
                "limits": LIMITS,
                "signed": false,
            }
        })
    }

    /// The certificate as a page renders it, every sentence already written, so
    /// no surface can word the same completion differently.
    pub fn page(&self) -> serde_json::Value {
        use serde_json::json;

        let Some(basis) = self.change.completion.as_ref() else {
            return json!({
                "finished": false,
                // Where an unfinished change is — not an error.
                "unfinished": format!("This change is not finished. It is {}.",
                                      where_it_is(self.change)),
                "last_gate": self.evidence.map(|g| g.summary()),
                "checks_altered": self.weakened_says(),
            });
        };

        json!({
            "finished": true,
            "basis": basis.headline(),
            "checked": basis.is_checked(),
            // Present only where it applies.
            "unchecked": basis.unchecked_note(),
            "evidence": self.evidence.map(|r| json!({
                Origin::KEY: Origin::ExternallyObserved.as_str(),
                "commit": r.commit.as_ref().map(commit_sentence),
                // Absent rather than a sentence: no repository is a different
                // fact from an unrecorded commit.
                "no_commit": r.commit.is_none().then_some(
                    "No commit was recorded — this gate did not run in a repository."
                ),
                "commands": r.commands.iter().map(|c| json!({
                    // Verbatim, so a reviewer can paste it.
                    "command": c.command,
                    "shown": crate::core::text::clip(&c.command, PAGE_COMMAND_CHARS),
                    "truncated": c.command.chars().count() > PAGE_COMMAND_CHARS,
                    "outcome": c.outcome.headline(),
                    "passed": c.passed(),
                    "took_ms": c.duration_ms,
                    "output_bytes": c.output_bytes,
                })).collect::<Vec<_>>(),
            })),
            "no_evidence": self.evidence.is_none().then(|| self.no_evidence()),
            "commands_digest": self.commands_digest(),
            "checks_altered": self.weakened_says(),
            "claim": self.claim.map(|text| json!({
                Origin::KEY: Origin::SelfReported.as_str(),
                "text": text,
                "caveat": "The agent\u{2019}s own account, shown beside the outcomes and never in \
                           place of them. Nothing here grades it against the evidence.",
            })),
            // Unsigned is a position, stated beside the flag; no signing option exists.
            "signing": {
                "signed": false,
                "says": "Unsigned on purpose — a signature would attest that this tool wrote \
                         this, which is not the claim.",
            },
            "limits": LIMITS,
            "rederivable": REDERIVABLE,
        })
    }

    /// The document, held to [`MAX_BYTES`]: truncates and says so, naming the
    /// command that gives the whole record.
    pub fn markdown_bounded(&self) -> String {
        let full = self.markdown();
        if full.len() <= MAX_BYTES {
            return full;
        }
        let note = format!(
            "\n---\n\n**This certificate was truncated to fit its size limit.** What is above is \
             incomplete. Read the structured form for the whole record: `devplane change export {} \
             --json`.",
            self.change.id.as_str()
        );
        let keep = MAX_BYTES.saturating_sub(note.len() + 1);
        let mut cut = keep;
        while cut > 0 && !full.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}\n{note}", &full[..cut])
    }
}

/// Where an unfinished change is: its state, and what it waits on.
fn where_it_is(change: &Change) -> String {
    let state = change.current_state();
    match change
        .waiting
        .as_ref()
        .map(crate::core::change::Waiting::says)
    {
        Some(waiting) => format!("{} {} — {waiting}", state.glyph(), state.as_str()),
        None => format!("{} {}", state.glyph(), state.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_four_outcomes_read_differently() {
        let base = CommandResult {
            command: "x".into(),
            outcome: Outcome::Exited { code: 1 },
            duration_ms: 0,
            output_tail: String::new(),
            output_bytes: 0,
            output_digest: String::new(),
            failures: Vec::new(),
        };
        let mut seen = std::collections::HashSet::new();
        for outcome in [
            Outcome::Exited { code: 1 },
            Outcome::TimedOut { after_secs: 600 },
            Outcome::NeverStarted {
                reason: "no such file".into(),
            },
            Outcome::Unknown {
                reason: "killed by a signal".into(),
            },
        ] {
            let c = CommandResult {
                outcome,
                ..base.clone()
            };
            assert!(
                seen.insert(outcome_sentence(&c)),
                "two outcomes produced the same sentence"
            );
        }
    }
}
