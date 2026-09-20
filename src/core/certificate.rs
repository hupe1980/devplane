//! The done certificate: evidence a reviewer can check without trusting this
//! tool.
//!
//! # Why this is not an attestation
//!
//! Software supply chain already has a mature answer to *"can a third party
//! believe this artifact was produced the way it claims?"* — in-toto statements
//! carrying SLSA provenance, signed in a DSSE envelope. It is worth being
//! precise about what that promises, because this looks like the same object and
//! is not. SLSA's own specification says of the builder field that it
//!
//! > identifies the build platform that executed the invocation, which is
//! > trusted to have correctly performed the operation
//!
//! and tells consumers they must accept only specific signer-builder pairs. A
//! signature proves the attestation was not altered; it proves nothing about
//! whether the claim inside it is true. **The producer is inside the trust
//! boundary by construction.**
//!
//! This certificate does not ask to be believed. It names a commit and a set of
//! commands so the reader runs them. The producer is outside the trust boundary
//! because the producer's assertion is not load-bearing — the reader's own shell
//! is. That is only possible because the domain is small: a gate is a handful of
//! commands against a commit, not an entire build platform. Where SLSA must
//! attest because re-running a build is infeasible, this can instruct, because
//! re-running a gate is a paste.
//!
//! So the statement *shape* is borrowed — it costs four keys and puts this in a
//! form anybody in that space already has a parser for — and the envelope and
//! the signature are refused. A signature needs a key, a key needs a trust root,
//! and a trust root needs an account, which this product does not have and does
//! not want. Unsigned is also the honest position: signing would invite exactly
//! the reading the artifact exists to avoid.
//!
//! # What it establishes
//!
//! That these commands ended as recorded against this commit. Not that the work
//! is correct, not that the commands check the right things, and not that the
//! record was never altered. The artifact says all of that in its own text,
//! because a statement of limits that lives in documentation does not travel
//! with a paste.

use super::work::{CommandResult, CommitStamp, Completion, GateReport, Outcome, Reach, Work};

/// The predicate type this emits. Versioned, because a consumer that cannot
/// tell which shape it is holding is a consumer that guesses.
pub const PREDICATE_TYPE: &str = "https://devplane.dev/DoneCertificate/v1";
/// The in-toto statement type, so the outer envelope is recognisable.
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";

/// The artifact's hard ceiling. A certificate nobody will read is not evidence,
/// and "bounded" with no number attached is not a requirement anybody can check.
pub const MAX_BYTES: usize = 64 * 1024;

/// What the artifact claims, in its own words, so it travels with the paste.
pub const LIMITS: &str = "This is evidence that these commands ended as recorded against this \
commit. It is not evidence that the work is correct, that the commands check the right things, or \
that the specification was satisfied. It is unsigned and detects change rather than forgery.";

/// Every field a reader can check for themselves, named once so both renderings
/// agree by construction rather than by discipline.
///
/// Two lists of which fields are checkable would be two homes for one fact, and
/// the one that drifts is the one nobody is looking at.
pub const REDERIVABLE: &[&str] = &[
    "evidence.commit.commit",
    "evidence.commands[].command",
    "evidence.commands[].outcome",
    "specification.fingerprint",
    "specification.tasks_done",
    "specification.tasks_total",
];

/// The portable record of one piece of work's completion.
///
/// Assembled from a `Work` and never stored: export is a command, not a schema.
pub struct Certificate<'a> {
    pub work: &'a Work,
    /// The gate run the basis points at, when it points at one.
    pub evidence: Option<&'a GateReport>,
    /// How many gate runs there were in total.
    pub attempts: u32,
    /// The agent's own account, when the rules allow it to be shown at all.
    pub claim: Option<&'a str>,
}

impl<'a> Certificate<'a> {
    /// Builds the certificate for a piece of work.
    ///
    /// The evidence is the run the **basis names**, not merely the last one. On
    /// a work finished by hand after a failure those are the same report and it
    /// does not matter; on a work whose basis points at attempt 2 of 4 it
    /// matters a great deal, and picking "the last one" would quietly show a
    /// different run from the one the completion rests on.
    pub fn of(work: &'a Work, claim: Option<&'a str>) -> Self {
        let evidence = match work.completion.as_ref().and_then(Completion::gate) {
            Some((gate, attempt)) => work
                .gates
                .iter()
                .find(|g| g.gate == gate && g.attempt == attempt)
                .or_else(|| work.last_gate()),
            None => work.last_gate(),
        };
        Self {
            work,
            evidence,
            attempts: work.gates.len() as u32,
            claim,
        }
    }

    /// Whether this work is finished at all.
    ///
    /// Exporting unfinished work is a legitimate question with an honest
    /// answer, so this is a branch in the rendering rather than an error.
    pub fn is_finished(&self) -> bool {
        self.work.completion.is_some()
    }

    /// The commands a reader runs to check this, in order.
    ///
    /// **This is the differentiator as data.** SLSA has nothing like it because
    /// it cannot: re-running a build platform is not a paste.
    pub fn verification_steps(&self) -> Vec<String> {
        let mut out = Vec::new();
        let Some(report) = self.evidence else {
            return out;
        };
        // **Where to get it, before what to do with it.** A commit with no
        // repository beside it is an instruction nobody can follow, and the
        // omission survived every test until somebody read the page.
        // `checkable_by_others` is the predicate this page turns on and it
        // lives on the stamp, where it is tested. The clone line asks for the
        // remote; the sentence below asks the same question a second way, and
        // the two used to be able to disagree.
        if let Some(url) = report
            .commit
            .as_ref()
            .filter(|c| c.checkable_by_others())
            .and_then(|c| c.remote.as_deref())
        {
            out.push(format!("git clone {url} && cd $(basename {url} .git)"));
        }
        if let Some(sha) = report.commit.as_ref().and_then(|c| c.commit.as_deref()) {
            out.push(format!("git checkout {sha}"));
        }
        out.extend(report.commands.iter().map(|c| c.command.clone()));
        out
    }

    /// Why a reader may not be able to follow those steps, if they cannot.
    ///
    /// A certificate that confidently instructs somebody to do something
    /// impossible is worse than one that says nothing.
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

/// How a command's outcome reads, and whether it is a verdict.
///
/// One function, so the document and the structured shape cannot describe one
/// state two ways.
pub fn outcome_sentence(c: &CommandResult, expect_fail: bool) -> String {
    match &c.outcome {
        // A reproduction inverts the meaning of an exit code, and printing
        // "exit 1 — failed" for a successful reproduction is lying in the
        // tidiest possible way.
        Outcome::Exited { code } if expect_fail && *code != 0 => {
            format!("exit {code} — the expected failure; this reproduced the problem")
        }
        Outcome::Exited { code } if expect_fail => {
            format!("exit {code} — did NOT reproduce the problem, which is the failure here")
        }
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
    let head = match &c.commit {
        Some(sha) => format!("Commit `{sha}`"),
        None => "No commit — this repository had no commits yet".to_string(),
    };
    let branch = match &c.branch {
        Some(b) => format!(" on branch `{b}`"),
        None => " on a detached head".to_string(),
    };
    let tree = match c.clean {
        true => "Working tree: clean — this commit is what was checked.".to_string(),
        false => format!(
            "Working tree: **{} uncommitted or untracked file(s)** — this commit does NOT fully \
             describe what was checked.",
            c.changed_files
        ),
    };
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

/// A fence long enough that the content cannot close it.
///
/// **This is the one place untrusted text could restructure the document.**
/// Gate commands come from a project's committed configuration, titles from
/// issues and models, output from whatever ran. In the structured shape the
/// serialiser handles all of it; in Markdown nothing does it for you, and a
/// command containing three backticks would end its own block and let the rest
/// of somebody else's bytes be read as markup.
///
/// So the fence is chosen *against the content* rather than fixed and hoped
/// for: one backtick longer than the longest run inside it, with a floor of
/// three. The page solved the same problem with a type that made escaping
/// structural; fencing by convention is that code before the fix.
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

/// Untrusted text on one line of a table or a sentence.
///
/// Pipes would open a new cell and newlines would end the row, so both are
/// neutralised. Backticks are left alone because the value is not being placed
/// inside a code span.
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
    // Padding only where it is needed: a backtick at either end would otherwise
    // fuse with the delimiter. Adding it unconditionally is harmless to a
    // renderer and noise to anybody reading the raw document.
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
        let w = self.work;

        o.push_str(&format!("# Done certificate — {}\n\n", inline(&w.title)));

        let Some(basis) = w.completion.as_ref() else {
            // Exporting unfinished work is a fair question, and the honest
            // answer is where it is — not an error, and not a certificate.
            o.push_str(&format!(
                "**This work is not finished.** It is at `{}`.\n\n",
                w.phase.as_str()
            ));
            if let Some(g) = self.evidence {
                o.push_str(&format!("Its last gate said: {}\n\n", inline(&g.summary())));
            } else {
                o.push_str("No gate has run.\n\n");
            }
            o.push_str("There is no completion to certify yet.\n");
            return o;
        };

        o.push_str(&format!("**Basis:** {}\n\n", basis.headline()));
        if !basis.is_checked() {
            o.push_str(
                "> Nothing was checked by this tool for this completion. Read the basis \
                        above before reading anything below as a pass.\n\n",
            );
        }

        o.push_str("## What was checked\n\n");
        match self.evidence {
            None => o.push_str("No gate ran, so there is no evidence to show.\n\n"),
            Some(report) => {
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
                        inline(&outcome_sentence(c, report.expect_fail)),
                        c.duration_ms as f64 / 1000.0,
                        bytes_phrase(c.output_bytes),
                        c.output_digest,
                    ));
                }
                o.push('\n');
                if report.commands.iter().any(|c| c.output_bytes > 0) {
                    o.push_str(
                        "Output shown anywhere below is a **tail**; the byte counts above say how \
                         much there was.\n\n",
                    );
                }
            }
        }

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
                    "The work says it answers {} and there was no such file or folder.\n\n",
                    code(&spec.path)
                )),
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
        let w = self.work;
        let commit = self.evidence.and_then(|r| r.commit.as_ref());
        // `subject` is what the statement is *about*. A commit is exactly the
        // kind of thing it is for.
        let subject = match commit.and_then(|c| c.commit.as_deref()) {
            Some(sha) => json!([{ "name": w.title, "digest": { "gitCommit": sha } }]),
            None => json!([{ "name": w.title, "digest": {} }]),
        };
        json!({
            "_type": STATEMENT_TYPE,
            "subject": subject,
            "predicateType": PREDICATE_TYPE,
            "predicate": {
                "work": { "id": w.id, "title": w.title, "phase": w.phase.as_str(),
                          "branch": w.branch },
                "completion": w.completion,
                "evidence": self.evidence.map(|r| json!({
                    "gate": r.gate,
                    "at": r.at,
                    "attempt": r.attempt,
                    "attempts": self.attempts,
                    "expect_fail": r.expect_fail,
                    "passed": r.passed(),
                    "commit": r.commit,
                    "commands": r.commands,
                })),
                "specification": self.evidence.and_then(|r| r.spec.clone()),
                "agent_claim": self.claim,
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

    /// The document, held to the ceiling.
    ///
    /// Shrinks what it shows rather than refusing, and says that it did — a
    /// certificate that silently drops evidence is the failure this whole
    /// feature is arranged against.
    pub fn markdown_bounded(&self) -> String {
        let full = self.markdown();
        if full.len() <= MAX_BYTES {
            return full;
        }
        let keep = MAX_BYTES.saturating_sub(TRUNCATION_NOTE.len() + 1);
        let mut cut = keep;
        while cut > 0 && !full.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}\n{TRUNCATION_NOTE}", &full[..cut])
    }
}

const TRUNCATION_NOTE: &str = "\n---\n\n**This certificate was truncated to fit its size limit.** \
What is above is incomplete. Read the structured form for the whole record.";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reproduction_never_reads_as_a_failure() {
        let c = CommandResult {
            command: "cargo test repro".into(),
            outcome: Outcome::Exited { code: 1 },
            duration_ms: 1,
            output_tail: String::new(),
            output_bytes: 0,
            output_digest: String::new(),
            failures: Vec::new(),
        };
        let s = outcome_sentence(&c, true);
        assert!(s.contains("reproduced the problem"), "{s}");
        assert!(!s.contains("failed"), "{s}");
    }

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
                seen.insert(outcome_sentence(&c, false)),
                "two outcomes produced the same sentence"
            );
        }
    }
}
