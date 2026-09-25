//! `devplane gate run`: an exit code and a sentence a workflow relies on. A failure,
//! an unchecked repository and an unreadable config must each be distinguishable
//! from a pass.

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

/// The command's JSON output, as the skill reads it.
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

/// The human-readable output, which must agree with the JSON.
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

/// A named gate is declared, validated by `devplane check`, listed, and run by name.
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

[gates.named.failing]
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
    assert!(run(&["gate", "run"]).status.success(), "check");
    assert!(
        run(&["gate", "run", "--name", "docs"]).status.success(),
        "a declared named gate runs"
    );

    // A named gate whose command fails is a failed gate.
    assert!(
        !run(&["gate", "run", "--name", "failing"]).status.success(),
        "a named gate whose command fails does not pass"
    );

    // An undeclared gate is an error, never a silent pass.
    let out = run(&["gate", "run", "--name", "nope"]);
    assert!(!out.status.success(), "an unknown gate must not pass");
    let said = String::from_utf8_lossy(&out.stdout);
    assert!(said.contains("no such gate"), "{said}");
    assert!(
        said.contains("docs") && said.contains("failing"),
        "it names what is declared, so the person can see the spelling: {said}"
    );
}
