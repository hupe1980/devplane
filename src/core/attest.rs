//! The attested statement: an in-toto Statement v1 whose predicate is the
//! in-toto **Test Result** predicate (v0.1), about the tree the `check` gate
//! ran on. A verifier that has never heard of Devplane reads `result`,
//! `configuration`, `passedTests` and `failedTests`; everything in-toto has no
//! slot for travels in one extension object, `predicate.devplane`, whose
//! presence or absence changes the meaning of no other field.
//!
//! `result` is the `check` gate's exit codes and nothing else: a weakened
//! check, a count, a basis or a person's mark never moves it, and `WARNED` is
//! never produced. The statement says so in its own text.
//!
//! Devplane never signs. The person may, with their own SSH key, at export
//! (`src/sign.rs`); the statement's bytes then say `signed: false` because a
//! detached signature is made after them. [`Recipe`] is the plain
//! `ssh-keygen -Y verify` a stranger runs.
//!
//! Pure: built from the record and values the edge read (the authority
//! counts, the `devplane.toml` blob id). Composed on read, never stored.

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::certificate::shell_quote;
use super::change::{CommandResult, CommitStamp, GateReport, Outcome};

/// The statement layer's `_type`.
pub const STATEMENT_TYPE: &str = "https://in-toto.io/Statement/v1";

/// The predicate: in-toto's Test Result, v0.1.
pub const TEST_RESULT_TYPE: &str = "https://in-toto.io/attestation/test-result/v0.1";

/// The SSH signature namespace for a certificate: not git's `git`, so a
/// certificate signature is never a commit signature, nor the reverse.
pub const SIGNING_NAMESPACE: &str = "certificate@devplane.dev";

/// The name the gate definition descriptor carries.
pub const DEFINITION_NAME: &str = "devplane.toml#gates.check";

/// How the definition digest's input is formed, in words a reader follows.
pub const CANONICAL_FORM: &str = "each command, in order, followed by one NUL byte";

/// How many weakened rows the statement lists before it counts the rest.
pub const MAX_ROWS: usize = 200;

/// What the predicate says about itself, first, before the limits.
pub const SAYS: &str = "`result` is the exit status of the `check` gate's commands against the \
subject tree: PASSED when every command exited 0, FAILED otherwise. It does not say the change is \
correct, safe or good.";

/// Why there is no statement. Each says so in one sentence; a statement with
/// an empty or placeholder subject is never made instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotAttestable {
    /// No `check` gate has run.
    NoCheckRun,
    /// The basis names a run the record no longer holds.
    BasisRunMissing { gate: String, attempt: u32 },
    /// The gate did not run in a repository, so there is no tree.
    NoRepository,
    /// The run recorded no working-tree digest; why, when known.
    NoTreeDigest { why: Option<String> },
    /// What was recorded as the tree is not a git object id.
    NotAnObjectId { tree: String },
    /// A command holds a NUL byte, which the definition digest cannot express.
    UnrepresentableCommand,
}

impl NotAttestable {
    /// The one sentence, shared by the CLI, the route and `devplane verify`.
    pub fn says(&self) -> String {
        let tree = TREE_PREFIX;
        match self {
            NotAttestable::NoCheckRun => "No in-toto statement: no `check` gate has run on this \
                 change, so there is nothing to attest."
                .into(),
            NotAttestable::BasisRunMissing { gate, attempt } => format!(
                "No in-toto statement: the basis names `{gate}` attempt {attempt}, and that run is \
                 no longer in the record."
            ),
            NotAttestable::NoRepository => {
                format!("{tree} no gate ran in a repository, so there is no tree to name.")
            }
            NotAttestable::NoTreeDigest { why } => format!(
                "{tree} no working-tree digest was recorded{}.",
                why.as_deref()
                    .map(|w| format!(" — {}", w.trim_end_matches('.')))
                    .unwrap_or_default()
            ),
            NotAttestable::NotAnObjectId { tree: t } => format!(
                "{tree} the recorded tree `{}` is not a git object id.",
                super::text::clip(t, 80)
            ),
            NotAttestable::UnrepresentableCommand => "No in-toto statement: a `check` command \
                 holds a NUL byte, which the gate definition digest cannot express."
                .into(),
        }
    }
}

impl std::fmt::Display for NotAttestable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.says())
    }
}

const TREE_PREFIX: &str =
    "No in-toto statement: a statement names the tree the commands ran against, and";

/// A git object id as the in-toto digest set spells one: lowercase hex, 40
/// (SHA-1) or 64 (SHA-256) characters.
pub fn is_object_id(s: &str) -> bool {
    matches!(s.len(), 40 | 64) && s.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The SHA-256 of the gate definition: each command's UTF-8 bytes followed by
/// one NUL byte, in order, as lowercase hex. A command holding a NUL cannot be
/// told apart from two, so it is refused rather than hashed ambiguously.
pub fn definition_digest(commands: &[String]) -> Result<String, NotAttestable> {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    for c in commands {
        if c.contains('\0') {
            return Err(NotAttestable::UnrepresentableCommand);
        }
        h.update(c.as_bytes());
        h.update([0u8]);
    }
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The shell line that prints [`definition_digest`] with stock tools.
pub fn recompute_line(commands: &[String]) -> String {
    let args: Vec<String> = commands.iter().map(|c| shell_quote(c)).collect();
    format!("printf '%s\\0' {} | shasum -a 256", args.join(" "))
}

/// The subject: the tree the commands ran against, named by `gitTree`; the
/// commit beside it in the same digest set only when the tree was clean, so
/// the commit's tree is the tree that was checked.
fn subject(name: &str, commit: Option<&CommitStamp>) -> Result<Value, NotAttestable> {
    let Some(c) = commit else {
        return Err(NotAttestable::NoRepository);
    };
    let tree = match c.tree.as_deref().filter(|t| !t.is_empty()) {
        None => {
            return Err(NotAttestable::NoTreeDigest {
                why: c.tree_unknown.clone(),
            });
        }
        Some(t) if !is_object_id(t) => {
            return Err(NotAttestable::NotAnObjectId { tree: t.into() });
        }
        Some(t) => t,
    };
    let mut digest = json!({ "gitTree": tree });
    if let Some(sha) = c.commit.as_deref().filter(|s| c.clean && is_object_id(s)) {
        digest["gitCommit"] = json!(sha);
    }
    // `_` is the Statement spec's name for *not meaningful*.
    let name = match name.trim() {
        "" => "_",
        n => n,
    };
    Ok(json!([{ "name": name, "digest": digest }]))
}

/// What the commands' outcomes give, with nothing else as an input.
fn passed(c: &CommandResult) -> bool {
    matches!(c.outcome, Outcome::Exited { code: 0 })
}

/// The one Test Result `result`: `PASSED` exactly when there were commands
/// and every one exited 0.
pub fn result_of(report: &GateReport) -> &'static str {
    match !report.commands.is_empty() && report.commands.iter().all(passed) {
        true => "PASSED",
        false => "FAILED",
    }
}

/// The statement for one gate run. The Test Result fields are computed from
/// `report` and `config_blob` alone; `extension` is placed beside them as
/// `predicate.devplane` and read by nothing here.
pub fn statement(
    name: &str,
    report: &GateReport,
    config_blob: Option<&str>,
    extension: Map<String, Value>,
) -> Result<Value, NotAttestable> {
    let subject = subject(name, report.commit.as_ref())?;
    let commands: Vec<String> = report.commands.iter().map(|c| c.command.clone()).collect();
    let mut configuration = vec![json!({
        "name": DEFINITION_NAME,
        "digest": { "sha256": definition_digest(&commands)? },
        "annotations": {
            "canonicalForm": CANONICAL_FORM,
            "recompute": recompute_line(&commands),
        },
    })];
    if let Some(blob) = config_blob.filter(|b| is_object_id(b)) {
        configuration.push(json!({ "name": "devplane.toml", "digest": { "gitBlob": blob } }));
    }
    let (ok, failed): (Vec<_>, Vec<_>) = report.commands.iter().partition(|c| passed(c));
    Ok(json!({
        "_type": STATEMENT_TYPE,
        "subject": subject,
        "predicateType": TEST_RESULT_TYPE,
        "predicate": {
            "result": result_of(report),
            "configuration": configuration,
            "passedTests": ok.iter().map(|c| &c.command).collect::<Vec<_>>(),
            "failedTests": failed.iter().map(|c| &c.command).collect::<Vec<_>>(),
            "devplane": Value::Object(extension),
        },
    }))
}

/// The statement's bytes: pretty JSON with every object's keys in sorted
/// order and one trailing newline, so an unchanged record exports the same
/// bytes every time, whichever order it was built or parsed in (a dependency
/// turns on `serde_json`'s `preserve_order`, so the order is not left to it).
pub fn serialize(v: &Value) -> Vec<u8> {
    let mut out = serde_json::to_vec_pretty(&sorted(v)).unwrap_or_default();
    out.push(b'\n');
    out
}

/// `v` with each object's keys re-inserted in sorted order.
fn sorted(v: &Value) -> Value {
    match v {
        Value::Object(o) => {
            let mut keys: Vec<&String> = o.keys().collect();
            keys.sort();
            Value::Object(
                keys.into_iter()
                    .map(|k| (k.clone(), sorted(&o[k])))
                    .collect(),
            )
        }
        Value::Array(a) => Value::Array(a.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

// ── The extension's parts ──────────────────────────────────────────────────

/// `devplane.commands[]`: each command in run order, with its exit code only
/// when it has one, and the reason in words when it has not.
pub fn commands_of(report: &GateReport) -> Value {
    Value::Array(
        report
            .commands
            .iter()
            .map(|c| {
                let (outcome, code) = match &c.outcome {
                    Outcome::Exited { code } => ("exited", Some(*code)),
                    Outcome::TimedOut { .. } => ("timedOut", None),
                    Outcome::NeverStarted { .. } => ("neverStarted", None),
                    Outcome::Unknown { .. } => ("unknown", None),
                };
                let mut o = json!({
                    "command": c.command,
                    "outcome": outcome,
                    "outcomeSays": super::certificate::outcome_sentence(c),
                    "elapsedMs": c.duration_ms,
                    "outputBytes": c.output_bytes,
                    "outputDigest": c.output_digest,
                });
                if let Some(code) = code {
                    o["exitCode"] = json!(code);
                }
                o
            })
            .collect(),
    )
}

/// `devplane.run`: which run, of how many, when; and, for a tree that held
/// uncommitted files, the commit it sat on, said not to be what was checked.
pub fn run_of(report: &GateReport, attempts: u32) -> Value {
    let mut run = json!({
        "gate": report.gate,
        "attempt": report.attempt,
        "attempts": attempts,
        "at": report.at,
        "commit": report.commit,
    });
    if let Some(c) = report.commit.as_ref().filter(|c| !c.clean)
        && let Some(sha) = c.commit.as_deref()
    {
        run["satOn"] = json!({ "commit": sha, "says": "not what was checked" });
    }
    run
}

/// `devplane.checksAltered`: the rows, listed and never scored, capped with
/// the omitted count stated; or `known: false` with why — never an empty
/// list standing in for *unknown*.
pub fn checks_altered(
    rows: Option<&[super::review::Weakened]>,
    all_seen: Option<bool>,
    says: String,
) -> Value {
    let Some(rows) = rows else {
        return json!({ "known": false, "says": says });
    };
    let mut o = json!({
        "known": true,
        "says": says,
        "rows": rows.iter().take(MAX_ROWS).map(|r| json!({
            "path": r.path, "kind": r.kind, "matched": r.matched, "why": r.why,
        })).collect::<Vec<_>>(),
    });
    if rows.len() > MAX_ROWS {
        o["omitted"] = json!(rows.len() - MAX_ROWS);
    }
    if let Some(seen) = all_seen {
        o["allSeen"] = json!(seen);
    }
    o
}

/// `devplane.permissionsDecidedBy`: a count per authority that decided at
/// least one of this change's tool calls. No name, no time, no rate.
pub fn permissions_of(counts: &BTreeMap<super::Authority, u32>) -> Value {
    Value::Object(
        counts
            .iter()
            .filter(|(_, n)| **n > 0)
            .map(|(a, n)| (a.as_str().to_string(), json!(n)))
            .collect(),
    )
}

// ── The person's signature ─────────────────────────────────────────────────

/// How a stranger checks a signed statement with `ssh-keygen` alone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// The principal on the allowed-signers line: the person's e-mail, or
    /// `signer`.
    pub identity: String,
    /// `<keytype> <base64>` of the signing key's public half.
    pub public_key: String,
    /// The statement's file name, as the commands refer to it.
    pub file: String,
}

/// What a reader is told about the key the line names.
pub const TRUST_NOTE: &str = "The allowed-signers line is the key the signature claims. Check it \
against a copy of the signer's public key you obtained yourself (for example, the keys their forge \
lists for them).";

impl Recipe {
    /// `principal namespaces="…" keytype base64`.
    pub fn allowed_signers_line(&self) -> String {
        format!(
            "{} namespaces=\"{SIGNING_NAMESPACE}\" {}",
            self.identity, self.public_key
        )
    }

    /// The commands, runnable as printed from a plain shell in the directory
    /// holding the statement and its `.sig`.
    pub fn commands(&self) -> Vec<String> {
        let f = shell_quote(&self.file);
        let sig = shell_quote(&format!("{}.sig", self.file));
        vec![
            format!(
                "printf '%s\\n' {} > allowed_signers",
                shell_quote(&self.allowed_signers_line())
            ),
            format!(
                "ssh-keygen -Y verify -f allowed_signers -I {} -n {SIGNING_NAMESPACE} -s {sig} < {f}",
                shell_quote(&self.identity)
            ),
        ]
    }

    /// The *Check the signature* section, as the certificate's Markdown words it.
    pub fn markdown(&self) -> String {
        format!(
            "## Check the signature\n\n{}\n\n{TRUST_NOTE}\n",
            super::certificate::fenced(&self.commands().join("\n"))
        )
    }

    /// The same, as JSON.
    pub fn json(&self, statement: &str, signature: &str) -> Value {
        json!({
            "statement": statement,
            "signature": signature,
            "namespace": SIGNING_NAMESPACE,
            "allowedSigners": self.allowed_signers_line(),
            "verify": self.commands(),
            "trust": TRUST_NOTE,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::change::Reach;

    fn stamp(clean: bool) -> CommitStamp {
        CommitStamp {
            commit: Some("a".repeat(40)),
            tree: Some("b".repeat(40)),
            tree_unknown: None,
            branch: Some("b".into()),
            clean,
            changed_files: if clean { 0 } else { 2 },
            reach: Reach::Remote,
            remote: None,
        }
    }

    fn report(commands: Vec<CommandResult>, commit: Option<CommitStamp>) -> GateReport {
        GateReport {
            junit: Vec::new(),
            base_commit: None,
            gate: "check".into(),
            at: jiff::Timestamp::UNIX_EPOCH,
            duration_ms: 1,
            commands,
            attempt: 1,
            spec: None,
            commit,
        }
    }

    #[test]
    fn the_definition_digest_is_pinned() {
        // `printf '%s\0' 'cargo test' | shasum -a 256`, computed once.
        assert_eq!(
            definition_digest(&["cargo test".into()]).unwrap(),
            "4f3dfeee70cb5f070850a66953ec2100cda84446cb7fe685fad1902ed6cec310"
        );
        assert_eq!(
            definition_digest(&[]).unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            definition_digest(&["a\0b".into()]),
            Err(NotAttestable::UnrepresentableCommand)
        );
    }

    #[test]
    fn a_dirty_tree_names_the_commit_it_sat_on_as_not_what_was_checked() {
        let r = report(vec![CommandResult::exited("x", 0)], Some(stamp(false)));
        assert_eq!(run_of(&r, 1)["satOn"]["says"], "not what was checked");
        let st = statement("n", &r, None, Map::new()).unwrap();
        assert!(st["subject"][0]["digest"].get("gitCommit").is_none());
        let r = report(vec![CommandResult::exited("x", 0)], Some(stamp(true)));
        assert!(run_of(&r, 1).get("satOn").is_none());
    }

    #[test]
    fn the_rows_are_capped_and_the_statement_stays_under_its_ceiling() {
        let rows: Vec<_> = (0..10_000)
            .map(|i| super::super::review::Weakened {
                path: format!("tests/t{i}.rs"),
                why: "adds a skip marker".into(),
                kind: super::super::review::WeakKind::Skip,
                matched: "#[ignore]".into(),
            })
            .collect();
        let altered = checks_altered(Some(&rows), Some(false), "s".into());
        assert_eq!(altered["rows"].as_array().unwrap().len(), MAX_ROWS);
        assert_eq!(altered["omitted"], 10_000 - MAX_ROWS);
        let mut ext = Map::new();
        ext.insert("checksAltered".into(), altered);
        let r = report(vec![CommandResult::exited("x", 0)], Some(stamp(true)));
        let st = statement("n", &r, None, ext).unwrap();
        assert!(serialize(&st).len() <= super::super::certificate::MAX_BYTES);
    }

    #[test]
    fn keys_come_out_sorted_so_the_bytes_are_stable() {
        // Built in one order, or parsed in another: the same bytes.
        let v = json!({ "b": 1, "a": { "d": [{ "f": 1, "e": 2 }], "c": 3 } });
        let parsed: Value =
            serde_json::from_str(r#"{"a":{"c":3,"d":[{"e":2,"f":1}]},"b":1}"#).unwrap();
        assert_eq!(serialize(&v), serialize(&parsed));
        let text = String::from_utf8(serialize(&v)).unwrap();
        let at = |k: &str| text.find(&format!("\"{k}\"")).unwrap();
        assert!(
            at("a") < at("b") && at("c") < at("d") && at("e") < at("f"),
            "{text}"
        );
        assert!(text.ends_with("}\n"));
    }
}
