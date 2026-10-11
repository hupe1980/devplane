//! The done certificate: evidence a reviewer can check without trusting this
//! tool. Its structured shape is an in-toto Statement v1 with the Test Result
//! predicate ([`super::attest`]). Devplane never signs it: SLSA provenance asks
//! the reader to trust the builder, whereas this names a tree and the gate
//! commands so the reader re-runs them. The person may sign the statement with
//! their own SSH key, which says who put their name to it. It establishes only
//! that these commands ended as recorded against that tree — not that the
//! work is correct — and says so in its own text ([`LIMITS`]).

use super::change::{
    Change, ChangeState, CommandResult, CommitStamp, Completion, Declared, GateReport, Outcome,
    Reach,
};

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

pub use super::attest::{STATEMENT_TYPE, TEST_RESULT_TYPE};

/// The artifact's hard size ceiling.
pub const MAX_BYTES: usize = 64 * 1024;

/// How much of a command a page row shows; the copy carries the whole thing.
pub const PAGE_COMMAND_CHARS: usize = 120;

/// What the artifact claims, in its own words, so it travels with the paste.
pub const LIMITS: &str = "This is evidence that these commands ended as recorded against the \
working tree it names. It is not evidence that the work is correct, that the commands check the \
right things, or that the specification was satisfied. The commands ran code in this tree, including tests and \
build scripts the change may have edited; the tree was the same before and after the run, which \
cannot see an edit reverted during the run. The base checks put the base's tests back to see \
whether they still pass, and their runs are observed beside this evidence, never part of it. \
Devplane does not sign it; unless a person signed it with their own key, it detects change rather \
than forgery.";

/// Every field a reader can check for themselves — one list shared by both renderings.
pub const REDERIVABLE: &[&str] = &[
    "subject[0].digest.gitTree",
    "subject[0].digest.gitCommit",
    "configuration[0].digest.sha256",
    "commands[].command",
    "commands[].exitCode",
    "specification.fingerprint",
    "specification.tasks_done",
    "specification.tasks_total",
    "baseChecks.base",
    "baseChecks.tree",
    "baseChecks.onChange.restored",
    "baseChecks.onBase.placed[].blob",
    "baseChecks.onChange.commands[].outcome",
    "baseChecks.onBase.commands[].outcome",
    "mergeApprovals.bearing[].state",
    "mergeApprovals.bearing[].commit",
];

/// The portable record of one change's completion. Assembled from a `Change`,
/// never stored.
pub struct Certificate<'a> {
    pub change: &'a Change,
    /// The gate run the basis points at, when it points at one.
    pub evidence: Option<&'a GateReport>,
    /// How many times `check` ran on this change ([`Change::check_attempts`]).
    pub attempts: u32,
    /// Where the change is, from the checks the project declares now.
    pub state: ChangeState,
    /// The agent's own account, when the rules allow it to be shown at all.
    pub claim: Option<&'a str>,
    /// The checks the change itself altered, read from its diff. `None` when
    /// the worktree is gone — said, never rendered as *nothing altered*.
    pub weakened: Option<Vec<super::review::Weakened>>,
    /// Why the diff could not be read, when it could not: `weakened` is then
    /// `None`, said as unknown, never as *nothing altered*.
    pub weakened_unreadable: Option<String>,
    /// Every weakened row was marked seen by a person before this was read.
    pub weakened_read: bool,
    /// The counts and base checks, as the change's own view composed them;
    /// `None` when `check` never ran. Beside the evidence, never in it.
    pub base_checks: Option<super::base_checks::View>,
    /// How many of this change's tool calls each authority decided, read by
    /// the edge; empty when none were, `None` when it was not read.
    pub decided_by: Option<std::collections::BTreeMap<super::Authority, u32>>,
    /// `devplane.toml`'s git blob id in the checked commit, when the tree was
    /// clean and committed and holds one.
    pub config_blob: Option<String>,
}

/// What the certificate says when the diff was read and nothing in it altered
/// the checks, what governs the agent or what the build trusts.
pub const NOTHING_ALTERED: &str = "This change altered none of its checks: no skip marker added, \
     no test file deleted, no gate or CI definition edited, and nothing that governs the agent \
     or the build edited.";

impl<'a> Certificate<'a> {
    /// Builds the certificate for a change. The evidence is the run the basis
    /// names, not the latest; if that run is gone the evidence is absent (no
    /// fallback to another run). With no basis yet, the latest `check` report.
    /// `declared` is what the project declares done to mean now, which the
    /// state is read against.
    pub fn of(change: &'a Change, declared: Declared<'_>, claim: Option<&'a str>) -> Self {
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
            attempts: change.check_attempts(),
            state: change.state(declared, change.tree_now.as_ref()),
            claim,
            weakened: None,
            weakened_unreadable: None,
            weakened_read: false,
            base_checks: None,
            decided_by: Default::default(),
            config_blob: None,
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
            None => match &self.weakened_unreadable {
                Some(why) => format!(
                    "Whether this change altered its own checks is unknown: {} ({why}).",
                    super::review::UNREADABLE
                ),
                None => "Whether this change altered its own checks could not be read: its \
                         worktree is gone."
                    .into(),
            },
            Some(w) if self.weakened_read && !w.is_empty() => format!(
                "{} A person marked each of these read.",
                super::review::weakened_sentence(w).unwrap_or_default()
            ),
            Some(w) => {
                super::review::weakened_sentence(w).unwrap_or_else(|| NOTHING_ALTERED.into())
            }
        }
    }

    /// A digest of the evidence's gate commands, in order.
    pub fn commands_digest(&self) -> Option<String> {
        self.evidence.map(commands_digest)
    }

    /// Whether this change is finished. Unfinished is a rendering branch, not an error.
    pub fn is_finished(&self) -> bool {
        self.change.completion.is_some()
    }

    /// Whether the tree the evidence ran on held uncommitted or untracked
    /// files, so its commit alone is not what was checked. Agents do not
    /// commit, so this is the ordinary case.
    fn uncommitted(&self) -> bool {
        self.evidence
            .and_then(|r| r.commit.as_ref())
            .is_some_and(|c| !c.clean)
    }

    /// The commands a reader runs to check this, in order. A clean tree is
    /// its commit, checked out by name. An uncommitted one is found by its
    /// working-tree digest instead: checking out the commit it sat on would
    /// re-run the commands on a different tree.
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
        match report.commit.as_ref() {
            Some(c) if c.clean => {
                if let Some(sha) = c.commit.as_deref() {
                    out.push(format!("git checkout {sha}"));
                }
            }
            Some(c) => {
                // A digest is hex; anything else is not pasted into a shell.
                if let Some(tree) = c
                    .tree
                    .as_deref()
                    .filter(|t| !t.is_empty() && t.chars().all(|ch| ch.is_ascii_hexdigit()))
                {
                    out.push(format!(
                        "c=$(git log --all --format='%H %T' | awk '$2 == \"{tree}\" {{ print $1; exit }}')"
                    ));
                    out.push(format!(
                        "git checkout \"${{c:?no commit has tree {tree}; the checked tree was never committed}}\""
                    ));
                }
            }
            None => {}
        }
        out.extend(report.commands.iter().map(|c| c.command.clone()));
        out
    }

    /// Why a reader may not be able to follow those steps, if they cannot.
    pub fn verification_caveat(&self) -> Option<String> {
        let commit = self.evidence.and_then(|r| r.commit.as_ref())?;
        let reach = self.reach_caveat(commit);
        if !self.uncommitted() {
            return reach;
        }
        let tree = match commit.tree.as_deref() {
            Some(tree) => format!(
                "The tree these commands ran on had uncommitted or untracked files, so no commit \
                 named here is that tree, and checking out the commit it sat on would run them \
                 on a different one. The steps look for a commit whose tree is `{tree}` (the \
                 working-tree digest); if none has it, the checked tree was never committed and \
                 cannot be re-run from the repository."
            ),
            None => "The tree these commands ran on had uncommitted or untracked files, and no \
                     working-tree digest was recorded, so nothing names the tree that was checked \
                     and these steps cannot reproduce it."
                .into(),
        };
        Some(match reach {
            Some(r) => format!("{tree} {r}"),
            None => tree,
        })
    }

    /// Whether others can reach the commit the evidence names.
    fn reach_caveat(&self, commit: &CommitStamp) -> Option<String> {
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

/// A digest of a report's gate commands, in order.
pub fn commands_digest(report: &GateReport) -> String {
    let mut h = super::hash::Rolling::new();
    for c in &report.commands {
        h.push_str(&c.command);
    }
    h.hex()
}

/// What stands in for a statement that cannot be made: no `_type`, so nothing
/// reads it as one, and the sentence saying why.
pub const NO_STATEMENT: &str = "no_statement";

/// What `devplane verify` knows when it writes a statement: a gate run and the
/// rows its diff weakened, with no change record behind them. The statement is
/// the certificate's own shape — one format, two callers — with a `verify`
/// block naming the base whose definition ran.
pub struct VerifyEvidence<'a> {
    /// The subject's name: the repository or checkout.
    pub name: &'a str,
    pub report: &'a GateReport,
    pub weakened: &'a [super::review::Weakened],
    /// `{ base, baseCommit, head, definition, definitionChanged }`.
    pub verify: serde_json::Value,
    /// `predicate.devplane.baseChecks`, as [`base_checks_json`] writes it.
    pub base_checks: serde_json::Value,
    /// The base's `devplane.toml` as a git blob id: the definition that ran.
    pub config_blob: Option<&'a str>,
}

/// The in-toto statement for a `devplane verify` run. Never signed: CI holds
/// no person's key. With no tree recorded there is no subject and so no
/// statement: the error says why.
pub fn verify_statement(e: &VerifyEvidence<'_>) -> Result<serde_json::Value, String> {
    use super::attest;
    let mut ext = serde_json::Map::new();
    ext.insert("says".into(), format!("{} {LIMITS}", attest::SAYS).into());
    ext.insert("run".into(), attest::run_of(e.report, 1));
    ext.insert("commands".into(), attest::commands_of(e.report));
    let says = super::review::weakened_sentence(e.weakened)
        .unwrap_or_else(|| "This change altered none of its checks.".into());
    ext.insert(
        "checksAltered".into(),
        attest::checks_altered(Some(e.weakened), None, says),
    );
    ext.insert("verify".into(), e.verify.clone());
    ext.insert("baseChecks".into(), e.base_checks.clone());
    ext.insert("rederivable".into(), REDERIVABLE.into());
    ext.insert("signed".into(), false.into());
    attest::statement(e.name, e.report, e.config_blob, ext).map_err(|n| n.says())
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
        (Some(sha), None) => match &c.tree_unknown {
            Some(why) => format!(
                "Commit `{sha}`, working-tree digest not recorded — {why} — so this run cannot \
                 verify anything"
            ),
            None => format!(
                "Commit `{sha}`, working-tree digest not recorded — the tree moved while the \
                 gate ran, or could not be read, so this run cannot verify anything"
            ),
        },
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
pub(crate) fn fenced(content: &str) -> String {
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
    /// The document a reviewer reads, sized for a pull request body. It ends
    /// with the change's `devplane://` link and the command that opens the
    /// same change, as plain text: whether a pull request or a chat makes the
    /// link clickable is theirs, and the command works either way.
    pub fn markdown(&self) -> String {
        let mut o = self.body();
        o.push_str(&self.link_lines());
        o
    }

    /// The last two lines: the link, and the command.
    fn link_lines(&self) -> String {
        let id = self.change.id.as_str();
        format!("\ndevplane://change/{id}\n\ndevplane change show {id}\n")
    }

    fn body(&self) -> String {
        let mut o = String::new();
        let w = self.change;

        o.push_str(&format!("# Done certificate — {}\n\n", inline(&w.title)));

        let Some(basis) = w.completion.as_ref() else {
            // Unfinished: say where it is — not an error, not a certificate.
            o.push_str(&format!(
                "**This change is not finished.** It is {}.\n\n",
                where_it_is(w, self.state)
            ));
            if let Some(g) = self.evidence {
                o.push_str(&format!("Its last gate said: {}\n\n", inline(&g.summary())));
            } else {
                o.push_str("No gate has run.\n\n");
            }
            o.push_str(&format!("{}\n\n", inline(&self.weakened_says())));
            o.push_str(&self.base_checks_markdown());
            o.push_str(&merge_approvals_markdown(self.change));
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
            let same = match self.uncommitted() {
                true => {
                    "Only once a commit with that tree is checked out should you see the same \
                         outcomes."
                }
                false => "You should see the same outcomes.",
            };
            o.push_str(&format!(
                "\n\n{same} You will probably **not** see the same output digests — see *What \
                 this does not establish*.\n\n"
            ));
        }

        // Beside the evidence and its steps, never part of them.
        o.push_str(&self.base_checks_markdown());
        o.push_str(&merge_approvals_markdown(self.change));

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

    /// Why this certificate's evidence cannot be attested, if it cannot.
    fn not_attestable(&self) -> Option<super::attest::NotAttestable> {
        use super::attest::NotAttestable;
        if self.evidence.is_some() {
            return None;
        }
        // The run the basis names is gone: never another run in its place.
        Some(
            match self
                .change
                .completion
                .as_ref()
                .and_then(Completion::gate)
                .filter(|_| self.basis_run_missing())
            {
                Some((gate, attempt)) => NotAttestable::BasisRunMissing {
                    gate: gate.into(),
                    attempt,
                },
                None => NotAttestable::NoCheckRun,
            },
        )
    }

    /// The structured shape: an in-toto Statement v1 with the Test Result
    /// predicate, about the tree `check` ran on. With nothing to attest there
    /// is no statement, and the error says why.
    pub fn statement(&self) -> Result<serde_json::Value, super::attest::NotAttestable> {
        use super::attest;
        use serde_json::json;
        if let Some(why) = self.not_attestable() {
            return Err(why);
        }
        let Some(report) = self.evidence else {
            return Err(attest::NotAttestable::NoCheckRun);
        };
        let w = self.change;
        let mut ext = serde_json::Map::new();
        ext.insert("says".into(), format!("{} {LIMITS}", attest::SAYS).into());
        let mut change = json!({
            "id": w.id, "title": w.title, "finished": self.is_finished(),
            "state": self.state.as_str(),
        });
        if let Some(b) = &w.branch {
            change["branch"] = json!(b);
        }
        ext.insert("change".into(), change);
        if let Some(c) = &w.completion {
            ext.insert("completion".into(), json!(c));
        }
        let mut run = attest::run_of(report, self.attempts);
        if let Some(basis) = &w.completion {
            run["basis"] = json!(basis.headline());
        }
        // Observed by something the agent does not control.
        run[Origin::KEY] = json!(Origin::ExternallyObserved.as_str());
        ext.insert("run".into(), run);
        ext.insert("commands".into(), attest::commands_of(report));
        ext.insert(
            "checksAltered".into(),
            attest::checks_altered(
                self.weakened.as_deref(),
                Some(self.weakened_read),
                self.weakened_says(),
            ),
        );
        ext.insert("baseChecks".into(), self.base_checks_json());
        ext.insert(
            "mergeApprovals".into(),
            merge_approvals_json(self.change.pull_request.as_ref()),
        );
        // Not read is said, never rendered as *nobody decided anything*.
        match &self.decided_by {
            Some(counts) => {
                ext.insert(
                    "permissionsDecidedBy".into(),
                    attest::permissions_of(counts),
                );
            }
            None => {
                ext.insert(
                    "permissionsUnknown".into(),
                    "The decision log was not read, so who decided this change's tool calls \
                     is not stated."
                        .into(),
                );
            }
        }
        if let Some(spec) = &report.spec {
            ext.insert("specification".into(), json!(spec));
        }
        // An object so the origin rides with the text; absent entirely when
        // the agent said nothing.
        if let Some(text) = self.claim {
            ext.insert(
                "agentClaim".into(),
                json!({ Origin::KEY: Origin::SelfReported.as_str(), "text": text }),
            );
        }
        let mut steps = self.verification_steps();
        if let Some(curl) = self.change.pull_request.as_ref().and_then(reviews_request) {
            steps.push(curl);
        }
        let mut verification = json!({ "steps": steps });
        if let Some(c) = self.verification_caveat() {
            verification["caveat"] = json!(c);
        }
        ext.insert("verification".into(), verification);
        ext.insert("rederivable".into(), REDERIVABLE.into());
        // These bytes carry no signature; a person's is a file beside them.
        ext.insert("signed".into(), false.into());
        attest::statement(&w.title, report, self.config_blob.as_deref(), ext)
    }

    /// The structured shape as the export carries it: the statement, or an
    /// object that is not one — `{"no_statement": why}`.
    pub fn json(&self) -> serde_json::Value {
        self.statement()
            .unwrap_or_else(|why| serde_json::json!({ NO_STATEMENT: why.says() }))
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
                                      where_it_is(self.change, self.state)),
                "last_gate": self.evidence.map(|g| g.summary()),
                "checks_altered": self.weakened_says(),
                "base_checks": self.base_checks_page(),
                "merge_approvals_says": merge_approvals_says(self.change.pull_request.as_ref()),
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
            "base_checks": self.base_checks_page(),
            "merge_approvals_says": merge_approvals_says(self.change.pull_request.as_ref()),
            "commands_digest": self.commands_digest(),
            "checks_altered": self.weakened_says(),
            "claim": self.claim.map(|text| json!({
                Origin::KEY: Origin::SelfReported.as_str(),
                "text": text,
                "caveat": "The agent\u{2019}s own account, shown beside the outcomes and never in \
                           place of them. Nothing here grades it against the evidence.",
            })),
            // Devplane never signs; the person may, with their own key.
            "signing": {
                "signed": false,
                "says": format!("Unsigned. Devplane never signs; you can sign the in-toto \
                                 statement with your own SSH key: devplane change export {} \
                                 --sign ssh --out <file>", self.change.id.as_str()),
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
        // The link and the command survive the cut: they are how a reader
        // reaches the whole record.
        let links = self.link_lines();
        let full = self.body();
        let note = format!(
            "\n---\n\n**This certificate was truncated to fit its size limit.** What is above is \
             incomplete. Read the structured form for the whole record: `devplane change export {} \
             --json`.",
            self.change.id.as_str()
        );
        let keep = MAX_BYTES.saturating_sub(note.len() + links.len() + 2);
        let mut cut = keep;
        while cut > 0 && !full.is_char_boundary(cut) {
            cut -= 1;
        }
        format!("{}\n{note}\n{links}", &full[..cut])
    }
}

impl Certificate<'_> {
    /// The one sentence about how many tests ran, after the commands table.
    fn counts_sentence(&self, report: &GateReport) -> String {
        match &self.base_checks {
            // The view's, when it describes this run.
            Some(v) if self.change.check_report().map(|r| r.at) == Some(report.at) => {
                v.certificate.clone()
            }
            _ => super::testcount::compare::certificate_sentence(&report.commands, None),
        }
    }

    /// Why there is no current base-checks result, as a sentence; `None` when
    /// there is one.
    fn base_checks_absent(&self) -> Option<String> {
        base_checks_absent(self.base_checks.as_ref())
    }

    /// The *Base checks* section: both runs, what was put back and placed,
    /// the named tests, and the steps to redo them — or the one sentence
    /// saying why there is nothing current. Never omitted.
    fn base_checks_markdown(&self) -> String {
        let mut o = String::from("## Base checks\n\n");
        // How many ran, as the runner printed it: first, since it needs no base run.
        if let Some(report) = self.evidence {
            o.push_str(&format!("{}\n\n", inline(&self.counts_sentence(report))));
        }
        if let Some(why) = self.base_checks_absent() {
            o.push_str(&format!("{}\n\n", inline(&why)));
            return o;
        }
        let Some(v) = &self.base_checks else {
            return o;
        };
        let (Some(on_change), Some(on_base)) = (&v.on_change, &v.on_base) else {
            return o;
        };
        let base = v.base.clone().unwrap_or_default();
        let tree = v.tree.clone().unwrap_or_default();
        o.push_str(&format!(
            "`{}: {}` — beside the evidence above, not part of it.\nBase commit {}; tree {} (the \
             tree `check` verified).\n\n",
            Origin::KEY,
            Origin::ExternallyObserved.as_str(),
            code(&base),
            code(&tree),
        ));
        let list = |items: &[String], more: usize| -> String {
            let mut s = items.iter().map(|i| code(i)).collect::<Vec<_>>().join(", ");
            if more > 0 {
                s.push_str(&format!(" and {more} more"));
            }
            s
        };
        let table = |o: &mut String, cmds: &[CommandResult]| {
            if cmds.is_empty() {
                return;
            }
            o.push_str("| Command | Outcome | Tests |\n|---|---|---|\n");
            for c in cmds {
                o.push_str(&format!(
                    "| {} | {} | {} |\n",
                    code(&c.command),
                    inline(&outcome_sentence(c)),
                    inline(&c.tests.says())
                ));
            }
            o.push('\n');
        };

        o.push_str("### The base's tests on this change\n\n");
        o.push_str(&format!("**{}.**\n\n", inline(&on_change.says)));
        if !on_change.restored.is_empty() {
            o.push_str(&format!(
                "Put back as the base has them: {}.\n\n",
                list(&on_change.restored, 0)
            ));
        }
        if !on_change.removed.is_empty() {
            o.push_str(&format!(
                "Removed, as the base lacks them: {}.\n\n",
                list(&on_change.removed, 0)
            ));
        }
        table(&mut o, &on_change.commands);
        if !on_change.failing.is_empty() {
            o.push_str(&format!(
                "Failing: {}.\n\n",
                list(&on_change.failing, on_change.failing_more)
            ));
        }
        if let Some(n) = &on_change.named {
            if !n.not_run_on_change.is_empty() {
                o.push_str(&format!(
                    "Did not run on this change: {}.\n\n",
                    list(&n.not_run_on_change, n.not_run_on_change_more)
                ));
            }
            if !n.became_skipped.is_empty() {
                o.push_str(&format!(
                    "Became skipped: {}.\n\n",
                    list(&n.became_skipped, n.became_skipped_more)
                ));
            }
        }

        o.push_str("### This change's new tests on the base\n\n");
        o.push_str(&format!("**{}.**\n\n", inline(&on_base.says)));
        if !on_base.placed.is_empty() {
            let placed: Vec<String> = on_base
                .placed
                .iter()
                .map(|p| match &p.blob {
                    Some(b) => format!("{} (blob {})", code(&p.path), code(b)),
                    None => code(&p.path),
                })
                .collect();
            o.push_str(&format!("Placed: {}.\n\n", placed.join(", ")));
        }
        if !on_base.not_separable.is_empty() {
            o.push_str(&format!(
                "Not separable: {}.\n\n",
                list(&on_base.not_separable, 0)
            ));
        }
        table(&mut o, &on_base.commands);
        if !on_base.passing.is_empty() {
            o.push_str(&format!(
                "Passing without the change: {}.\n\n",
                list(&on_base.passing, on_base.passing_more)
            ));
        }
        if !on_base.failing.is_empty() {
            o.push_str(&format!(
                "Failing without it: {}.\n\n",
                list(&on_base.failing, on_base.failing_more)
            ));
        }

        o.push_str("### To redo both by hand\n\n");
        let restored = on_change
            .restored
            .iter()
            .map(|p| shell_quote(p))
            .collect::<Vec<_>>()
            .join(" ");
        let placed = on_base
            .placed
            .iter()
            .map(|p| shell_quote(&p.path))
            .collect::<Vec<_>>()
            .join(" ");
        let mut steps = vec![
            format!("git worktree add --detach /tmp/base-checks {base}"),
            "cd /tmp/base-checks".to_string(),
            format!("git read-tree -u --reset {tree}"),
        ];
        if !restored.is_empty() {
            steps.push(format!("git checkout {base} -- {restored}"));
        }
        for p in &on_change.removed {
            steps.push(format!("rm -f {}", shell_quote(p)));
        }
        steps.push("# run the commands of the first table".into());
        steps.push(format!(
            "git read-tree -u --reset {base} && git clean -ffdx"
        ));
        if !placed.is_empty() {
            steps.push(format!("git checkout {tree} -- {placed}"));
            steps.push("# run the commands of the second table; compare each placed blob with `git hash-object <path>`".into());
        }
        o.push_str(&fenced(&steps.join("\n")));
        o.push_str("\n\n");
        o
    }

    /// `predicate.devplane.baseChecks`.
    fn base_checks_json(&self) -> serde_json::Value {
        base_checks_json(self.base_checks.as_ref(), self.base_checks_absent())
    }

    /// The page's sentences, as the host wrote them.
    fn base_checks_page(&self) -> serde_json::Value {
        serde_json::json!({
            "counts": self.base_checks.as_ref().map(|v| &v.counts_says),
            "on_change": self.base_checks.as_ref().and_then(|v| v.on_change.as_ref()).map(|o| &o.says),
            "on_base": self.base_checks.as_ref().and_then(|v| v.on_base.as_ref()).map(|o| &o.says),
            "absent": self.base_checks_absent(),
            "stale": self.base_checks.as_ref().is_some_and(|v| v.stale),
        })
    }
}

/// Where an unfinished change is: its state, and what it waits on.
fn where_it_is(change: &Change, state: ChangeState) -> String {
    match change
        .waiting
        .as_ref()
        .map(crate::core::change::Waiting::says)
    {
        Some(waiting) => format!("{} {} — {waiting}", state.glyph(), state.as_str()),
        None => format!("{} {}", state.glyph(), state.as_str()),
    }
}

/// Why there is no current base-checks result, as a sentence; `None` when
/// there is one.
pub fn base_checks_absent(view: Option<&super::base_checks::View>) -> Option<String> {
    match view {
        None => Some("Base checks not run — `check` has not run on this change.".into()),
        Some(v) => v.absent.as_ref().map(|a| {
            let says = match (a.absent, &v.tree) {
                (super::base_checks::Absent::Stale, Some(t)) => format!(
                    "base checks stale — they describe tree `{}`, not this one",
                    super::text::clip(t, 12)
                ),
                _ => a.says.clone(),
            };
            let mut c = says.chars();
            match c.next() {
                Some(f) => format!("{}{}.", f.to_uppercase(), c.as_str()),
                None => String::new(),
            }
        }),
    }
}

/// `predicate.devplane.mergeApprovals`: the reading of who approved the
/// change's pull request, or why there is none — never an empty list.
pub fn merge_approvals_json(pr: Option<&super::change::PullRequestRef>) -> serde_json::Value {
    use super::approvals::ReadingState;
    use serde_json::json;
    let Some(pr) = pr else {
        return json!({ "absent": "no pull request" });
    };
    match &pr.approvals {
        None => json!({ "unread": NOT_READ_YET }),
        Some(ReadingState::NotRead { why }) => json!({ "unread": why }),
        Some(ReadingState::Stale { last, why }) => json!({ "unread": why, "last": last }),
        Some(ReadingState::Current { reading }) => json!(reading),
    }
}

/// What a pull request whose approvals were never read says.
pub const NOT_READ_YET: &str = "the pull request's approvals have not been read yet";

/// The one sentence a surface shows for the approvals: the reading's, or why
/// there is none. `None` without a pull request.
pub fn merge_approvals_says(pr: Option<&super::change::PullRequestRef>) -> Option<String> {
    use super::approvals::ReadingState;
    let pr = pr?;
    Some(match &pr.approvals {
        None => format!("Approvals not read: {NOT_READ_YET}."),
        Some(ReadingState::NotRead { why }) => format!("Approvals not read: {why}."),
        Some(ReadingState::Stale { last, why }) => format!(
            "As of {}, not current ({why}): {}",
            last.read_at, last.sentence
        ),
        Some(ReadingState::Current { reading }) => reading.sentence.clone(),
    })
}

/// The documented request that re-reads the reviews, when the pull request's
/// address can be read as `https://<host>/<owner>/<repo>/pull/<n>`.
pub fn reviews_request(pr: &super::change::PullRequestRef) -> Option<String> {
    let rest = pr.url.strip_prefix("https://")?;
    let mut parts = rest.split('/');
    let (host, owner, repo, kind, n) = (
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
        parts.next()?,
    );
    let safe = |s: &str| {
        !s.is_empty()
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if kind != "pull"
        || n.parse::<u64>().ok()? != pr.number
        || ![host, owner, repo].iter().all(|s| safe(s))
    {
        return None;
    }
    let api = match host.eq_ignore_ascii_case("github.com") {
        true => "https://api.github.com".to_string(),
        false => format!("https://{host}/api/v3"),
    };
    Some(format!(
        "curl -H \"Accept: application/vnd.github+json\" {api}/repos/{owner}/{repo}/pulls/{}/reviews",
        pr.number
    ))
}

/// The *Merge approvals* section: absent without a pull request.
fn merge_approvals_markdown(change: &Change) -> String {
    use super::approvals::{ReadingState, sentence::LIMITS_LINE};
    let Some(pr) = change.pull_request.as_ref() else {
        return String::new();
    };
    let mut o = String::from("## Merge approvals\n\n");
    if let Some(says) = merge_approvals_says(Some(pr)) {
        o.push_str(&format!("{}\n\n", inline(&says)));
    }
    let reading = pr.approvals.as_ref().and_then(ReadingState::reading);
    if let Some(r) = reading {
        if let Some(rule) = &r.required_says {
            o.push_str(&format!("{}\n\n", inline(rule)));
        }
        o.push_str(&match r.after_merge {
            true => format!("Read at {}, after the pull request merged.\n\n", r.read_at),
            false => format!(
                "Read at {}. This is what the reviews said then, not necessarily at merge.\n\n",
                r.read_at
            ),
        });
    }
    if let Some(review) = pr.decision_quote() {
        o.push_str(&format!("GitHub says: {}.\n\n", inline(&review)));
    }
    if let Some(curl) = reviews_request(pr) {
        o.push_str(
            "To re-read the reviews (add an `Authorization` header for a private repository):\n\n",
        );
        o.push_str(&fenced(&curl));
        o.push_str("\n\n");
    }
    o.push_str(LIMITS_LINE);
    o.push_str("\n\n");
    o
}

/// `predicate.devplane.baseChecks`: the whole result when current, else
/// the sentence and which absence it is. Never `null`.
pub fn base_checks_json(
    view: Option<&super::base_checks::View>,
    absent_says: Option<String>,
) -> serde_json::Value {
    use serde_json::json;
    let origin = Origin::ExternallyObserved.as_str();
    let Some(v) = view else {
        return json!({
            Origin::KEY: origin,
            "absent": "check_not_run",
            "says": absent_says,
        });
    };
    if let Some(a) = &v.absent {
        return json!({
            Origin::KEY: origin,
            "absent": a.absent,
            "says": absent_says,
            "stale": v.stale,
        });
    }
    let on_change = v.on_change.as_ref();
    let on_base = v.on_base.as_ref();
    json!({
        Origin::KEY: origin,
        "says": {
            "counts": v.counts_says,
            "onChange": on_change.map(|o| &o.says),
            "onBase": on_base.map(|o| &o.says),
        },
        "stale": v.stale,
        "base": v.base,
        "tree": v.tree,
        "onChange": on_change.map(|o| json!({
            "restored": o.restored,
            "removed": o.removed,
            "inline": o.inline,
            "outcome": o.outcome,
            "commands": o.commands.iter().map(|c| json!({
                "command": c.command, "outcome": c.outcome, "tests": c.tests,
            })).collect::<Vec<_>>(),
            "failing": o.failing,
            "failingOmitted": o.failing_more,
            "named": o.named,
        })),
        "onBase": on_base.map(|o| json!({
            "placed": o.placed,
            "notSeparable": o.not_separable,
            "outcome": o.outcome,
            "commands": o.commands.iter().map(|c| json!({
                "command": c.command, "outcome": c.outcome, "tests": c.tests,
            })).collect::<Vec<_>>(),
            "passing": o.passing,
            "passingOmitted": o.passing_more,
            "failing": o.failing,
            "failingOmitted": o.failing_more,
        })),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing altered names the agent's controls and the build's inputs too;
    /// a lockfile row reaches the statement with its kind.
    #[test]
    fn the_controls_are_named_in_what_the_change_altered() {
        assert!(NOTHING_ALTERED.contains("no gate or CI definition edited"));
        assert!(NOTHING_ALTERED.contains("nothing that governs the agent or the build edited"));
        let lock = super::super::review::Weakened {
            path: "Cargo.lock".into(),
            why: "edits the lockfile `Cargo.lock`: 1 package added (`a` 1.0)".into(),
            kind: super::super::review::WeakKind::SupplyChain,
            matched: "changed lines 0".into(),
        };
        let rows = std::slice::from_ref(&lock);
        let says = super::super::review::weakened_sentence(rows).unwrap();
        let altered = super::super::attest::checks_altered(Some(rows), Some(false), says);
        assert_eq!(altered["rows"][0]["kind"], "supply_chain", "{altered}");
        assert!(
            altered["says"]
                .as_str()
                .unwrap()
                .starts_with("It also edited what governs the agent or what the build trusts"),
            "{altered}"
        );
    }

    #[test]
    fn the_four_outcomes_read_differently() {
        let base = CommandResult {
            tests: Default::default(),
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
