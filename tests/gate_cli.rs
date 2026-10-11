//! `devplane gate`: an exit code and a sentence a workflow relies on. A failure,
//! an unchecked repository and an unreadable config must each be distinguishable
//! from a pass.

mod common;

use std::path::PathBuf;
use std::process::Command;

/// A scratch directory with whatever `devplane.toml` the caller wants, or none.
fn scratch(tag: &str, config: Option<&str>) -> PathBuf {
    let dir = common::scratch(&format!("gate-{tag}"));
    if let Some(c) = config {
        std::fs::write(dir.join("devplane.toml"), c).unwrap();
    }
    dir
}

/// The command's JSON output, as the skill reads it.
fn gate_json(dir: &PathBuf) -> (serde_json::Value, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["gate", "--json", "--cwd"])
        .arg(dir)
        .output()
        .expect("the gate runs");
    let code = out
        .status
        .code()
        .expect("it exits rather than being killed");
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    let value = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("the gate answers with JSON ({e}); it said:\n{text}"));
    (value, code)
}

/// The human-readable output, which must agree with the JSON.
fn gate_text(dir: &PathBuf) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["gate", "--cwd"])
        .arg(dir)
        .output()
        .expect("the gate runs");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        out.status
            .code()
            .expect("it exits rather than being killed"),
    )
}

#[test]
fn a_failing_gate_names_the_command_and_what_it_exited_with() {
    // The failing gate is named so an agent need not guess which one.
    let dir = scratch(
        "failing",
        Some("[gates]\ncheck = [\"sh -c 'exit 3'\"]\ntimeout = \"30s\"\n"),
    );
    let (v, code) = gate_json(&dir);

    assert_eq!(code, 1, "a failing gate must not exit 0");
    assert_eq!(v["state"], "failed");
    assert_eq!(v["passed"], false);

    let commands = v["commands"].as_array().expect("the commands are reported");
    assert_eq!(
        commands.len(),
        1,
        "one command was declared, so one is reported"
    );
    let only = &commands[0];
    assert!(
        only["command"].as_str().unwrap().contains("exit 3"),
        "the failing command names itself: {only}"
    );

    // And how it failed: status, timeout and spawn failure need different fixes.
    let rendered = serde_json::to_string(only).unwrap();
    assert!(
        rendered.contains('3'),
        "the exit code appears in the reported outcome: {rendered}"
    );

    let (text, text_code) = gate_text(&dir);
    assert_eq!(text_code, code, "both renderings exit the same way");
    assert!(
        text.contains("exit 3"),
        "a person is told which command: {text}"
    );
    assert!(text.contains("failed"), "and that it failed: {text}");
}

#[test]
fn a_repository_with_no_gates_is_not_a_pass_and_never_says_passed() {
    // Nothing checked must never read as verified to a caller that reads only the
    // exit code.
    let nothing = scratch("nogates", None);
    let empty = scratch("emptygates", Some("[gates]\ncheck = []\n"));

    for dir in [&nothing, &empty] {
        let (v, code) = gate_json(dir);
        assert_eq!(code, 1, "an unchecked repository exits non-zero: {dir:?}");
        assert_eq!(v["state"], "no_gates");
        assert_eq!(v["passed"], false);
        assert!(
            v["commands"].as_array().unwrap().is_empty(),
            "nothing ran, so nothing is reported as having run"
        );

        let summary = v["summary"].as_str().unwrap();
        assert!(
            !summary.contains("passed"),
            "the sentence for an unchecked repository may not contain the word \
             `passed`, because a reader skimming for it will find it: {summary}"
        );

        let (text, text_code) = gate_text(dir);
        assert_eq!(text_code, 1);
        assert!(
            !text.contains("passed"),
            "and neither may what a person reads: {text}"
        );
    }
}

#[test]
fn a_configuration_that_will_not_parse_reports_the_parsers_error_and_runs_nothing() {
    // An unreadable file does not approve, as with an unparsable policy.
    let dir = scratch("broken", Some("[gates]\ncheck = [unclosed\n"));
    let (v, code) = gate_json(&dir);

    assert_eq!(code, 1);
    assert_eq!(v["state"], "config_unreadable");
    assert_eq!(v["passed"], false);
    assert!(
        v["commands"].as_array().unwrap().is_empty(),
        "nothing was run, so no command result may be reported: {v}"
    );

    // The parser's own error, with its line number.
    let error = v["error"].as_str().expect("the parser's error is carried");
    assert!(!error.is_empty());
    let (text, _) = gate_text(&dir);
    assert!(
        text.contains(error.lines().next().unwrap()),
        "a person is shown the same error: {text}"
    );

    // Distinct from `no_gates`: gates were declared but could not be read.
    assert_ne!(v["summary"], gate_json(&scratch("cmp", None)).0["summary"]);
}

/// A named gate is declared, validated by `devplane config check`, listed, and run by name.
#[test]
fn a_named_gate_can_be_run_by_name() {
    let dir = scratch(
        "named",
        Some(
            r#"
[project]
name = "named-gate"

[gates]
check = ["true"]

[gates.docs]
run = ["true"]

[gates.failing]
run = ["false"]
"#,
        ),
    );

    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(args)
            .arg("--cwd")
            .arg(&dir)
            .output()
            .expect("devplane")
    };

    // The default is still `check`, and naming a declared gate runs that one.
    assert!(run(&["gate"]).status.success(), "check");
    assert!(
        run(&["gate", "--name", "docs"]).status.success(),
        "a declared named gate runs"
    );

    // A named gate whose command fails is a failed gate.
    assert!(
        !run(&["gate", "--name", "failing"]).status.success(),
        "a named gate whose command fails does not pass"
    );

    // An undeclared gate is an error, never a silent pass.
    let out = run(&["gate", "--name", "nope"]);
    assert!(!out.status.success(), "an unknown gate must not pass");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("no such gate"), "{said}");
    assert!(
        said.contains("docs") && said.contains("failing"),
        "it names what is declared, so the person can see the spelling: {said}"
    );
}

// ── `devplane gate install` ──────────────────────────────────────────────────

/// A Spec Kit repository: `.specify/`, one passing gate, and a scratch home so
/// nothing of the person running the tests is read or written.
struct Installing {
    repo: PathBuf,
    home: PathBuf,
    /// What `PATH` holds: an empty directory, or one with a stub `specify`.
    bin: PathBuf,
}

impl Installing {
    fn new(tag: &str) -> Self {
        let repo = scratch(&format!("install-{tag}"), Some(common::PASSING));
        std::fs::create_dir_all(repo.join(".specify")).unwrap();
        let home = common::scratch(&format!("install-home-{tag}"));
        let bin = common::scratch(&format!("install-bin-{tag}"));
        Self { repo, home, bin }
    }

    /// Puts a `specify` on `PATH` that records its arguments and the manifest
    /// it was handed, and writes nothing into the repository.
    #[cfg(unix)]
    fn with_specify(self) -> Self {
        use std::os::unix::fs::PermissionsExt;
        let stub = self.bin.join("specify");
        std::fs::write(
            &stub,
            "#!/bin/sh\n\
             printf '%s\\n' \"$@\" > \"$STUB_LOG\"\n\
             /bin/cp \"$4/extension.yml\" \"$STUB_LOG.manifest\"\n\
             /bin/cp \"$4/overlays/devplane-gate.yml\" \"$STUB_LOG.overlay\"\n\
             echo 'stub specify: extension added'\n",
        )
        .unwrap();
        std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755)).unwrap();
        self
    }

    fn log(&self) -> PathBuf {
        self.home.join("specify.log")
    }

    fn extensions(&self) -> PathBuf {
        self.repo.join(".specify/extensions.yml")
    }

    /// Runs `devplane gate install <args>` in the repository: stdout, stderr, exit.
    fn run(&self, args: &[&str]) -> (String, String, i32) {
        let out = Command::new(env!("CARGO_BIN_EXE_devplane"))
            .args(["gate", "install"])
            .args(args)
            .current_dir(&self.repo)
            .env("PATH", &self.bin)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("STUB_LOG", self.log())
            .env("NO_COLOR", "1")
            .stdin(std::process::Stdio::null())
            .output()
            .expect("devplane runs");
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().expect("it exits"),
        )
    }
}

const THEIRS: &str = "# their comment, which a YAML round-trip would drop\n\
hooks:\n  before_plan:\n    - extension: theirs\n      command: theirs.thing\n";

#[test]
fn an_existing_extensions_file_survives_byte_for_byte() {
    // Without `specify`: the entry is printed to paste, the file is not written.
    let t = Installing::new("theirs");
    std::fs::write(t.extensions(), THEIRS).unwrap();
    let (out, err, code) = t.run(&["--speckit", "--anyway"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), THEIRS);
    assert!(
        out.contains("add this under `hooks.after_implement:`"),
        "{out}"
    );
    assert!(out.contains("command: devplane.gate"), "{out}");
    assert!(
        !out.contains("condition"),
        "no condition is ever offered: {out}"
    );

    // With `specify`: Spec Kit's own command owns the file; Devplane writes none of it.
    #[cfg(unix)]
    {
        let t = Installing::new("theirs-specify").with_specify();
        std::fs::write(t.extensions(), THEIRS).unwrap();
        let (out, err, code) = t.run(&["--speckit"]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), THEIRS);
        assert!(t.log().is_file(), "specify was run: {out}");
    }
}

#[test]
fn a_second_registration_changes_nothing_and_says_so() {
    // The hand-written entry, created when absent, then found.
    let t = Installing::new("twice");
    let (out, err, code) = t.run(&["--speckit", "--anyway"]);
    assert_eq!(code, 0, "{out}{err}");
    let first = std::fs::read_to_string(t.extensions()).expect("created when absent");
    assert!(first.contains("command: devplane.gate"), "{first}");
    assert!(!first.contains("condition"), "{first}");

    let (out, _, code) = t.run(&["--speckit", "--anyway"]);
    assert_eq!(code, 0);
    assert!(out.contains("Nothing to do"), "{out}");
    assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), first);

    // The extension's entry, as `specify extension add` writes it: found, and
    // `specify` is not run again.
    #[cfg(unix)]
    {
        let t = Installing::new("twice-specify").with_specify();
        let installed = "installed:\n- devplane\nhooks:\n  after_implement:\n  - extension: devplane\n    command: speckit.devplane.gate\n    enabled: true\n    optional: false\n";
        std::fs::write(t.extensions(), installed).unwrap();
        let (out, _, code) = t.run(&["--speckit"]);
        assert_eq!(code, 0);
        assert!(out.contains("Nothing to do"), "{out}");
        assert!(
            !t.log().exists(),
            "specify was run for an installed extension"
        );
        assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), installed);
    }
}

#[test]
fn an_extensions_file_that_will_not_parse_is_not_written() {
    let t = Installing::new("unparsable");
    let broken = "hooks: [this is not\n";
    std::fs::write(t.extensions(), broken).unwrap();
    let (out, err, code) = t.run(&["--speckit", "--anyway"]);
    assert_eq!(code, 0, "{out}{err}");
    assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), broken);
    assert!(
        out.contains("command: devplane.gate"),
        "the entry to paste: {out}"
    );
}

#[test]
fn it_refuses_without_spec_kit_and_for_an_unknown_event() {
    let t = Installing::new("refusals");
    std::fs::remove_dir_all(t.repo.join(".specify")).unwrap();
    let (out, err, code) = t.run(&["--speckit", "--anyway"]);
    assert_ne!(code, 0, "{out}");
    assert!(err.contains("Spec Kit is not installed here"), "{err}");
    assert!(!t.repo.join(".specify").exists(), "nothing was created");

    let t = Installing::new("event");
    let (out, err, code) = t.run(&["--speckit", "--anyway", "--event", "after_lunch"]);
    assert_ne!(code, 0, "{out}");
    assert!(err.contains("`after_lunch` is not a hook point"), "{err}");
    assert!(
        err.contains("after_implement") && err.contains("before_converge"),
        "{err}"
    );
    assert!(!t.extensions().exists(), "nothing was written");

    // And a repository with no gate is refused unless asked.
    let t = Installing::new("nogate");
    std::fs::remove_file(t.repo.join("devplane.toml")).unwrap();
    let (_, err, code) = t.run(&["--speckit"]);
    assert_ne!(code, 0);
    assert!(err.contains("declares no gate"), "{err}");
    assert!(!t.extensions().exists());
}

#[cfg(unix)]
#[test]
fn with_specify_on_path_spec_kit_installs_the_extension_and_the_file_is_not_hand_edited() {
    let t = Installing::new("specify").with_specify();

    // A dry run shows what would happen and runs nothing.
    let (out, err, code) = t.run(&["--speckit", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("specify extension add --dev"), "{out}");
    assert!(out.contains("command: speckit.devplane.gate"), "{out}");
    assert!(!t.log().exists(), "a dry run ran specify");
    assert!(!t.extensions().exists(), "a dry run wrote the file");

    // The real run hands Spec Kit the extension and writes nothing itself.
    let (out, err, code) = t.run(&["--speckit"]);
    assert_eq!(code, 0, "{out}{err}");
    let args = std::fs::read_to_string(t.log()).expect("specify was run");
    let args: Vec<&str> = args.lines().collect();
    assert_eq!(&args[..3], ["extension", "add", "--dev"], "{args:?}");
    assert!(
        !t.extensions().exists(),
        "Devplane wrote extensions.yml itself where Spec Kit's CLI owns it"
    );
    let manifest = std::fs::read_to_string(format!("{}.manifest", t.log().display())).unwrap();
    assert!(
        manifest.contains("name: \"speckit.devplane.gate\""),
        "{manifest}"
    );
    assert!(manifest.contains("optional: false"), "{manifest}");
    let overlay = std::fs::read_to_string(format!("{}.overlay", t.log().display())).unwrap();
    assert!(
        overlay.contains("run: \"devplane gate --json\""),
        "{overlay}"
    );
    // The deterministic route is named, for the person to add.
    assert!(out.contains("specify workflow overlay add"), "{out}");
    assert!(
        !t.repo.join(".specify/workflows").exists(),
        "no overlay added unasked"
    );

    // Another event moves the extension's hook with it.
    let t = Installing::new("specify-event").with_specify();
    let (out, err, code) = t.run(&["--speckit", "--event", "after_converge"]);
    assert_eq!(code, 0, "{out}{err}");
    let manifest = std::fs::read_to_string(format!("{}.manifest", t.log().display())).unwrap();
    assert!(manifest.contains("  after_converge:\n"), "{manifest}");
    assert!(!manifest.contains("  after_implement:\n"), "{manifest}");
}

#[cfg(unix)]
#[test]
fn a_hand_written_hook_beside_the_extension_is_named_and_left_alone() {
    let t = Installing::new("both").with_specify();
    let hand =
        "hooks:\n  after_implement:\n    - extension: devplane\n      command: devplane.gate\n";
    std::fs::write(t.extensions(), hand).unwrap();
    let (out, err, code) = t.run(&["--speckit", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("hand-written `devplane.gate` hook"), "{out}");
    assert!(out.contains("run twice"), "{out}");
    assert_eq!(std::fs::read_to_string(t.extensions()).unwrap(), hand);
}

// ── `devplane gate install --skill` ──────────────────────────────────────────

#[test]
fn the_verify_skill_is_offered_as_a_diff_and_written_only_on_confirm() {
    let t = Installing::new("skill");
    let skill = t.repo.join(".claude/skills/verify/SKILL.md");

    // Shown, as a diff of the whole new file, and nothing written.
    let (out, err, code) = t.run(&["--skill", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("Changes to"), "{out}");
    assert!(
        out.contains("+ name: verify"),
        "the diff adds the skill: {out}"
    );
    assert!(out.contains("+ devplane gate --json"), "{out}");
    assert!(!skill.exists(), "a dry run wrote the skill");

    // Nobody at a terminal to say yes: nothing is written.
    let (out, err, code) = t.run(&["--skill"]);
    assert_ne!(code, 0, "{out}");
    assert!(
        out.contains("+ name: verify"),
        "the diff is still shown: {out}"
    );
    assert!(err.contains("nobody can answer"), "{err}");
    assert!(!skill.exists(), "written without a confirmation");

    // Under --json likewise.
    let (out, _, code) = t.run(&["--skill", "--json"]);
    assert_ne!(code, 0);
    assert!(out.contains("nobody can confirm"), "{out}");
    assert!(!skill.exists());

    // Confirmed: written, and it is the skill that was shown.
    let (out, err, code) = t.run(&["--skill", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    let text = std::fs::read_to_string(&skill).expect("written on confirm");
    assert!(text.starts_with("---\nname: verify\n"), "{text}");
    assert!(
        text.contains("devplane gate install --skill"),
        "it names its origin"
    );
    assert!(text.contains("never *verified*"), "{text}");
    assert!(
        !t.home.join(".claude").exists(),
        "nothing went to the user's skills"
    );

    // Again: nothing to change.
    let (out, _, code) = t.run(&["--skill", "--yes"]);
    assert_eq!(code, 0);
    assert!(out.contains("Nothing to do"), "{out}");

    // `--user` puts it in the (scratch) home instead.
    let (out, err, code) = t.run(&["--skill", "--user", "--yes"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(t.home.join(".claude/skills/verify/SKILL.md").is_file());
}

#[test]
fn somebody_elses_verify_skill_is_not_replaced_on_a_flag() {
    let t = Installing::new("foreign");
    let skill = t.repo.join(".claude/skills/verify/SKILL.md");
    std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
    let theirs = "---\nname: verify\ndescription: Use when committing. Runs make check.\n---\n\nmake check\n";
    std::fs::write(&skill, theirs).unwrap();

    let (out, err, code) = t.run(&["--skill", "--yes"]);
    assert_ne!(code, 0, "{out}");
    assert!(err.contains("Devplane did not write"), "{err}");
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), theirs);

    // Asked for, the replacement is a diff that removes their lines.
    let (out, err, code) = t.run(&["--skill", "--anyway", "--dry-run"]);
    assert_eq!(code, 0, "{out}{err}");
    assert!(out.contains("- make check"), "{out}");
    assert_eq!(std::fs::read_to_string(&skill).unwrap(), theirs);
}

#[test]
fn the_verify_skill_is_refused_where_no_gate_is_declared() {
    let t = Installing::new("skill-nogate");
    std::fs::remove_file(t.repo.join("devplane.toml")).unwrap();
    let (_, err, code) = t.run(&["--skill", "--yes"]);
    assert_ne!(code, 0);
    assert!(err.contains("declares no gate"), "{err}");
    assert!(!t.repo.join(".claude").exists());
}
