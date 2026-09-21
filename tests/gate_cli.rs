//! `devplane gate run`: the verdict something outside this process relies on.
//!
//! The feature's whole contribution is an exit code and a sentence that a Spec
//! Kit workflow reads back to an agent. Both are asserted here against real
//! repositories, because the three ways this can go wrong — a failure that
//! reads as a pass, an unchecked repository that reads as a pass, and an
//! unreadable configuration that reads as either — are indistinguishable from
//! success to anything that only looks at whether the command ran.

use std::path::PathBuf;
use std::process::Command;

/// A scratch directory with whatever `devplane.toml` the caller wants, or none.
fn scratch(tag: &str, config: Option<&str>) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "vp-gate-{tag}-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    if let Some(c) = config {
        std::fs::write(dir.join("devplane.toml"), c).unwrap();
    }
    dir
}

/// The command under test, as JSON — the shape the skill reads.
fn gate_json(dir: &PathBuf) -> (serde_json::Value, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["gate", "run", "--json", "--cwd"])
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

/// And as a person reads it, because the two must agree.
fn gate_text(dir: &PathBuf) -> (String, i32) {
    let out = Command::new(env!("CARGO_BIN_EXE_devplane"))
        .args(["gate", "run", "--cwd"])
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
    // "Checks failed" is not a verdict anybody can act on, and an agent told
    // only that something failed will guess at which thing.
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

    // **And what it exited with**, which is the half most likely to be dropped:
    // the name alone tells a reader which command, and nothing about how it
    // failed — a non-zero status, a timeout and a shell that could not start it
    // are three different problems with three different next steps.
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
    // This is the assertion the feature is most likely to lose, and losing it is
    // exactly how *nothing was checked* comes to look verified to a workflow that
    // only reads the exit code.
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
    // A file that cannot be read is not a file that approves — the same rule the
    // permission path follows for a policy it could not parse.
    let dir = scratch("broken", Some("[gates]\ncheck = [unclosed\n"));
    let (v, code) = gate_json(&dir);

    assert_eq!(code, 1);
    assert_eq!(v["state"], "config_unreadable");
    assert_eq!(v["passed"], false);
    assert!(
        v["commands"].as_array().unwrap().is_empty(),
        "nothing was run, so no command result may be reported: {v}"
    );

    // **The parser's own error, not a paraphrase of it.** The person has to fix
    // a file, and the line number is the whole of the help.
    let error = v["error"].as_str().expect("the parser's error is carried");
    assert!(!error.is_empty());
    let (text, _) = gate_text(&dir);
    assert!(
        text.contains(error.lines().next().unwrap()),
        "a person is shown the same error: {text}"
    );

    // And the four states stay four: this one does not borrow `no_gates`'
    // sentence, which would tell a reader the repository declared no checks
    // when in fact it declared some that could not be read.
    assert_ne!(v["summary"], gate_json(&scratch("cmp", None)).0["summary"]);
}

/// **A named gate could be declared, validated and listed — and only a pipeline
/// step could run one.**
///
/// `[gates.named.x]` is checked by `devplane check`, printed by it, and
/// reachable from a pipeline. Nothing on the CLI could ask for it, so a person
/// who wrote one down had no way to try it before wiring a pipeline around it.
/// That is a configuration key with no reader for its commonest use, which is
/// the defect `devplane check` exists to complain about, pointed inward.
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

[gates.named.docs]
run = ["true"]

[gates.named.mustfail]
run = ["false"]
expect = "fail"
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
    assert!(run(&["gate", "run"]).status.success(), "check");
    assert!(
        run(&["gate", "run", "--name", "docs"]).status.success(),
        "a declared named gate runs"
    );

    // **`expect = "fail"` is honoured**, so a gate that did exactly what it was
    // asked to do is not reported as a failure.
    assert!(
        run(&["gate", "run", "--name", "mustfail"]).status.success(),
        "a gate declared `expect = \"fail\"` passes when its command fails"
    );

    // **An undeclared gate is an error, never a silent pass.** An empty gate
    // that reads as success is the failure this layer exists to prevent.
    let out = run(&["gate", "run", "--name", "nope"]);
    assert!(!out.status.success(), "an unknown gate must not pass");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("no such gate"), "{said}");
    assert!(
        said.contains("docs") && said.contains("mustfail"),
        "it names what is declared, so the person can see the spelling: {said}"
    );
}
