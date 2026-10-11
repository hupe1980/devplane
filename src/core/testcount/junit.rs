//! The JUnit XML a project's `check` writes, read to name tests. The file is
//! output of code the agent could edit, so it is parsed by a maintained reader
//! with no DTD and no entity beyond XML's five, and every name read from it is
//! text, never markup. The host reads the bytes; this only parses them.

use serde::{Deserialize, Serialize};

/// How one case ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseOutcome {
    Passed,
    Failed,
    Error,
    Skipped,
}

impl CaseOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            CaseOutcome::Passed => "passed",
            CaseOutcome::Failed => "failed",
            CaseOutcome::Error => "error",
            CaseOutcome::Skipped => "skipped",
        }
    }
}

/// One `<testcase>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Case {
    pub suite: String,
    pub class: String,
    pub name: String,
    /// Which occurrence of the same `(suite, class, name)`, from zero.
    pub nth: u32,
    /// The runner's `file` attribute, when it writes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub outcome: CaseOutcome,
}

impl Case {
    pub fn id(&self) -> CaseId {
        CaseId {
            suite: self.suite.clone(),
            class: self.class.clone(),
            name: self.name.clone(),
            nth: self.nth,
        }
    }
}

/// A case's identity across two runs.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct CaseId {
    pub suite: String,
    pub class: String,
    pub name: String,
    pub nth: u32,
}

impl CaseId {
    /// `tests.test_login::test_lockout`.
    pub fn says(&self) -> String {
        match self.class.is_empty() {
            true => self.name.clone(),
            false => format!("{}::{}", self.class.trim(), self.name.trim()),
        }
    }
}

/// What reading one declared JUnit path came to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "read", rename_all = "snake_case")]
pub enum JunitRead {
    Read {
        path: String,
        cases: Vec<Case>,
        /// The cases were emptied because a newer `check` report exists; its
        /// counts stay on the commands.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        dropped: bool,
    },
    /// Absent, or older than the gate's start: not written by this run.
    NotWritten {
        path: String,
    },
    /// Resolves outside the checkout.
    Outside {
        path: String,
    },
    TooLarge {
        path: String,
        bytes: u64,
    },
    Unparseable {
        path: String,
        why: String,
    },
}

impl JunitRead {
    pub fn cases(&self) -> &[Case] {
        match self {
            JunitRead::Read { cases, .. } => cases,
            _ => &[],
        }
    }

    /// Why nothing was named from this file, when nothing was.
    pub fn says(&self) -> Option<String> {
        match self {
            JunitRead::Read { .. } => None,
            JunitRead::NotWritten { path } => {
                Some(format!("`{path}` not written by this run — nothing named"))
            }
            JunitRead::Outside { path } => Some(format!(
                "`{path}` resolves outside the checkout, so it was not read"
            )),
            JunitRead::TooLarge { path, bytes } => Some(format!(
                "`{path}` is {bytes} bytes, over the {} it is read up to",
                "16 MiB"
            )),
            JunitRead::Unparseable { path, why } => {
                Some(format!("`{path}` could not be parsed: {why}"))
            }
        }
    }
}

/// One declared path, as read after a run, with whether git ignores it
/// (`Some(false)` is reported: writing it moves the tree `check` is judged on).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JunitFile {
    pub read: JunitRead,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ignored: Option<bool>,
}

impl JunitFile {
    /// What a surface says about this file: why nothing was named from it,
    /// and that git does not ignore it. Nothing when it was read and ignored.
    pub fn notes(&self) -> Vec<String> {
        let mut out: Vec<String> = self.read.says().into_iter().collect();
        if self.ignored == Some(false) {
            let path = match &self.read {
                JunitRead::Read { path, .. }
                | JunitRead::NotWritten { path }
                | JunitRead::Outside { path }
                | JunitRead::TooLarge { path, .. }
                | JunitRead::Unparseable { path, .. } => path,
            };
            out.push(format!(
                "`{path}` is not ignored by git, so writing it moves the tree `check` is judged \
                 against"
            ));
        }
        out
    }
}

/// The largest JUnit file read.
pub const MAX_BYTES: u64 = 16 * 1024 * 1024;

/// Parses one file's text. Pure: the host passes what it read.
pub fn parse(path: &str, xml: &str) -> JunitRead {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut suites: Vec<String> = Vec::new();
    let mut cases: Vec<Case> = Vec::new();
    // The case being read, when inside `<testcase>…</testcase>`.
    let mut open: Option<Case> = None;
    let mut seen: std::collections::HashMap<(String, String, String), u32> = Default::default();
    let unparseable = |why: String| JunitRead::Unparseable {
        path: path.to_string(),
        why,
    };
    loop {
        let event = match reader.read_event() {
            Ok(e) => e,
            Err(e) => return unparseable(e.to_string()),
        };
        let (e, empty) = match event {
            Event::Eof => break,
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                match e.local_name().as_ref() {
                    "testsuite" => {
                        suites.pop();
                    }
                    "testcase" => close(&mut open, &mut cases, &mut seen),
                    _ => {}
                }
                continue;
            }
            _ => continue,
        };
        let attrs = attributes(&e);
        match e.local_name().as_ref() {
            "testsuite" if !empty => suites.push(attrs.get("name").cloned().unwrap_or_default()),
            "testcase" => {
                open = Some(Case {
                    suite: suites.last().cloned().unwrap_or_default(),
                    class: attrs.get("classname").cloned().unwrap_or_default(),
                    name: attrs.get("name").cloned().unwrap_or_default(),
                    nth: 0,
                    file: attrs.get("file").cloned(),
                    outcome: CaseOutcome::Passed,
                });
                // `<testcase …/>` has no end tag: it closes where it opens.
                if empty {
                    close(&mut open, &mut cases, &mut seen);
                }
            }
            "failure" => set(&mut open, CaseOutcome::Failed),
            "error" => set(&mut open, CaseOutcome::Error),
            "skipped" => set(&mut open, CaseOutcome::Skipped),
            // nextest's retries of a test that then passed or failed: the
            // case's own outcome stands.
            _ => {}
        }
    }
    JunitRead::Read {
        path: path.to_string(),
        cases,
        dropped: false,
    }
}

/// Records `outcome` on the open case; a failure outranks a skip.
fn set(open: &mut Option<Case>, outcome: CaseOutcome) {
    if let Some(c) = open.as_mut()
        && !matches!(c.outcome, CaseOutcome::Failed | CaseOutcome::Error)
    {
        c.outcome = outcome;
    }
}

fn close(
    open: &mut Option<Case>,
    cases: &mut Vec<Case>,
    seen: &mut std::collections::HashMap<(String, String, String), u32>,
) {
    if let Some(mut c) = open.take() {
        let n = seen
            .entry((c.suite.clone(), c.class.clone(), c.name.clone()))
            .or_insert(0);
        c.nth = *n;
        *n += 1;
        cases.push(c);
    }
}

/// Attribute values, with XML's own five entities read and nothing else: an
/// unknown entity stays as written.
fn attributes(e: &quick_xml::events::BytesStart<'_>) -> std::collections::HashMap<String, String> {
    let mut out = std::collections::HashMap::new();
    for a in e.attributes().flatten() {
        let key = a.key.local_name().as_ref().to_string();
        let value = match a.normalized_value(quick_xml::XmlVersion::Implicit1_0) {
            Ok(v) => v.into_owned(),
            Err(_) => a.value.to_string(),
        };
        out.insert(key, value);
    }
    out
}

/// What differs between the base's tests on this change (`ran_here`) and the
/// change's own `check` (`change`), each list sorted and never truncated here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NamedDiff {
    /// Ran with the base's tests, absent from the change's `check`.
    pub not_run_on_change: Vec<CaseId>,
    /// Ran with the base's tests, skipped on the change.
    pub became_skipped: Vec<CaseId>,
    /// In both, with a different outcome: (id, with the base's tests, on the change).
    pub changed: Vec<(CaseId, CaseOutcome, CaseOutcome)>,
    /// In the change's `check`, absent with the base's tests.
    pub new: Vec<CaseId>,
}

impl NamedDiff {
    /// *9 did not run on this change · 1 became skipped*, zero parts omitted.
    pub fn says(&self) -> String {
        let parts: Vec<String> = [
            (self.not_run_on_change.len(), "did not run on this change"),
            (self.became_skipped.len(), "became skipped"),
            (self.changed.len(), "changed outcome"),
            (self.new.len(), "new on this change"),
        ]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .map(|(n, what)| format!("{n} {what}"))
        .collect();
        match parts.is_empty() {
            true => "the same tests ran, with the same outcomes".into(),
            false => parts.join(" · "),
        }
    }
}

/// Compares two runs' cases by `(suite, class, name, nth)`.
pub fn diff(ran_here: &[Case], change: &[Case]) -> NamedDiff {
    use std::collections::BTreeMap;
    let here: BTreeMap<CaseId, CaseOutcome> =
        ran_here.iter().map(|c| (c.id(), c.outcome)).collect();
    let there: BTreeMap<CaseId, CaseOutcome> = change.iter().map(|c| (c.id(), c.outcome)).collect();
    let mut out = NamedDiff::default();
    for (id, a) in &here {
        match there.get(id) {
            None => out.not_run_on_change.push(id.clone()),
            Some(CaseOutcome::Skipped) if *a != CaseOutcome::Skipped => {
                out.became_skipped.push(id.clone())
            }
            Some(b) if b != a => out.changed.push((id.clone(), *a, *b)),
            Some(_) => {}
        }
    }
    out.new = there
        .keys()
        .filter(|id| !here.contains_key(*id))
        .cloned()
        .collect();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const PYTEST: &str = include_str!("../../../tests/fixtures/runners/junit/pytest.xml");
    const PYTEST_K: &str = include_str!("../../../tests/fixtures/runners/junit/pytest_k.xml");
    const NEXTEST: &str = include_str!("../../../tests/fixtures/runners/junit/nextest.xml");
    const JEST: &str = include_str!("../../../tests/fixtures/runners/junit/jest.xml");
    const VITEST: &str = include_str!("../../../tests/fixtures/runners/junit/vitest.xml");
    const GOTESTSUM: &str = include_str!("../../../tests/fixtures/runners/junit/gotestsum.xml");

    fn cases(xml: &str) -> Vec<Case> {
        match parse("r.xml", xml) {
            JunitRead::Read { cases, .. } => cases,
            other => panic!("not read: {other:?}"),
        }
    }

    fn outcomes(cs: &[Case]) -> (usize, usize, usize, usize) {
        let n = |o: CaseOutcome| cs.iter().filter(|c| c.outcome == o).count();
        (
            n(CaseOutcome::Passed),
            n(CaseOutcome::Failed),
            n(CaseOutcome::Error),
            n(CaseOutcome::Skipped),
        )
    }

    #[test]
    fn every_recorded_writer_reads_to_its_cases() {
        let py = cases(PYTEST);
        assert_eq!(py.len(), 12);
        assert_eq!(outcomes(&py), (10, 1, 0, 1));
        let lockout = py.iter().find(|c| c.name == "test_lockout").unwrap();
        assert_eq!(lockout.class, "tests.test_login");
        assert_eq!(lockout.outcome, CaseOutcome::Failed);
        assert_eq!(lockout.file, None, "pytest writes no file attribute");

        let nx = cases(NEXTEST);
        assert_eq!(nx.len(), 7, "the ignored test is not written");
        assert!(nx.iter().any(|c| c.outcome == CaseOutcome::Failed));
        assert!(nx.iter().all(|c| c.suite.starts_with("rs")));

        let jest = cases(JEST);
        assert_eq!(outcomes(&jest).1, 1);
        assert!(
            jest.iter()
                .all(|c| c.file.as_deref() == Some("jt/a.test.js")),
            "jest-junit writes the file attribute"
        );

        let vt = cases(VITEST);
        assert_eq!(vt.len(), 5);
        assert_eq!(outcomes(&vt).1, 1);

        let go = cases(GOTESTSUM);
        assert!(
            go.iter()
                .any(|c| c.name == "TestFails" && c.outcome == CaseOutcome::Failed)
        );
        assert!(go.iter().any(|c| c.outcome == CaseOutcome::Skipped));
    }

    #[test]
    fn a_retry_record_is_not_the_outcome() {
        let xml = r#"<testsuites><testsuite name="s"><testcase name="t" classname="c">
            <flakyFailure message="first try"/></testcase></testsuite></testsuites>"#;
        assert_eq!(cases(xml)[0].outcome, CaseOutcome::Passed);
    }

    #[test]
    fn a_doctype_entity_is_never_expanded() {
        let xml = r#"<?xml version="1.0"?><!DOCTYPE x [<!ENTITY e SYSTEM "file:///etc/passwd">]>
            <testsuite name="s"><testcase name="&e;" classname="c"/></testsuite>"#;
        let cs = cases(xml);
        assert_eq!(cs.len(), 1);
        assert_eq!(cs[0].name, "&e;");
    }

    #[test]
    fn malformed_xml_is_unparseable_not_empty() {
        let got = parse("r.xml", "<testsuite><testcase name='a'></testsuite>");
        assert!(matches!(got, JunitRead::Unparseable { .. }), "{got:?}");
    }

    #[test]
    fn twelve_against_three_names_nine_sorted() {
        let all = cases(PYTEST);
        let three = cases(PYTEST_K);
        assert_eq!(three.len(), 3);
        let d = diff(&all, &three);
        assert_eq!(d.not_run_on_change.len(), 9);
        let mut sorted = d.not_run_on_change.clone();
        sorted.sort();
        assert_eq!(sorted, d.not_run_on_change);
        assert!(d.says().starts_with("9 did not run on this change"));
        // test_lockout failed in the twelve and passed in the three.
        assert_eq!(d.changed.len(), 1);
        assert_eq!(d.changed[0].0.name, "test_lockout");
    }

    #[test]
    fn a_pass_that_became_a_skip_is_said() {
        let case = |o| Case {
            suite: "s".into(),
            class: "c".into(),
            name: "t".into(),
            nth: 0,
            file: None,
            outcome: o,
        };
        let d = diff(&[case(CaseOutcome::Passed)], &[case(CaseOutcome::Skipped)]);
        assert_eq!(d.became_skipped.len(), 1);
        assert!(d.changed.is_empty());
    }

    #[test]
    fn duplicates_are_told_apart_by_occurrence() {
        let xml = r#"<testsuite name="s"><testcase name="t" classname="c"/>
            <testcase name="t" classname="c"><failure/></testcase></testsuite>"#;
        let cs = cases(xml);
        assert_eq!((cs[0].nth, cs[1].nth), (0, 1));
        assert_eq!(cs[1].outcome, CaseOutcome::Failed);
    }
}
