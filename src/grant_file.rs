//! Writing a grant into the person's own `~/.devplane/policy.toml`, and taking
//! one out. The file is theirs: they read, edit and delete it by hand, so an
//! edit here changes only the one entry and keeps every comment and every
//! other line as they wrote it.
//!
//! Written only by a person's act — the window's proved route, or `devplane
//! config revoke` in their own terminal — and, like `trusted.toml`, whole to a
//! sibling, owner-only, renamed into place. A file that will not read or parse
//! is never overwritten: the error says so and the person fixes it.

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::ops::Range;
use std::path::{Path, PathBuf};

/// The file's name under Devplane's home.
pub const FILE: &str = "policy.toml";

/// `<home>/policy.toml`.
pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

#[derive(Deserialize)]
struct Spans {
    #[serde(default)]
    projects: BTreeMap<String, ProjectSpans>,
}

#[derive(Deserialize)]
struct ProjectSpans {
    #[serde(default)]
    allow: Option<toml::Spanned<Vec<toml::Spanned<toml::Value>>>>,
}

fn rule_of(v: &toml::Value) -> Option<&str> {
    match v {
        toml::Value::String(s) => Some(s),
        toml::Value::Table(t) => t.get("rule").and_then(toml::Value::as_str),
        _ => None,
    }
}

/// The rules the file grants for `root`, or the reason it will not load.
fn granted(text: &str, root: &str) -> Result<Vec<String>> {
    let parsed = crate::core::GlobalConfig::parse(text)
        .map_err(|e| anyhow::anyhow!("{} will not load: {e}", crate::core::grant::FILE))?;
    Ok(parsed
        .projects
        .get(root)
        .map(|p| p.allow.iter().map(|e| e.rule().to_string()).collect())
        .unwrap_or_default())
}

fn read(home: &Path) -> Result<String> {
    let at = path(home);
    match std::fs::read_to_string(&at) {
        Ok(t) => Ok(t),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("reading {}", at.display())),
    }
}

/// The text with `rule` added to `[projects."<root>"] allow`. Pure, so the
/// edit is testable without a disk.
pub fn with_rule(text: &str, root: &str, rule: &str) -> Result<String> {
    let before = granted(text, root)?;
    if before.iter().any(|r| r == rule) {
        return Ok(text.to_string());
    }
    let spans: Spans = toml::from_str(text).context("reading the file's layout")?;
    let quoted = crate::core::grant::toml_string(rule);
    let mut out = text.to_string();
    match spans.projects.get(root) {
        Some(ProjectSpans {
            allow: Some(list), ..
        }) => {
            let whole: Range<usize> = list.span();
            match list.get_ref().last() {
                Some(last) => {
                    let end = last.span().end;
                    let rest = &text[end..whole.end];
                    let comma = rest
                        .find(|c: char| !c.is_whitespace())
                        .filter(|&i| rest.as_bytes().get(i) == Some(&b','));
                    match comma {
                        Some(i) => out.insert_str(end + i + 1, &format!("\n  {quoted},")),
                        None => out.insert_str(end, &format!(",\n  {quoted},")),
                    }
                }
                None => out.insert_str(whole.start + 1, &format!("\n  {quoted},\n")),
            }
        }
        Some(_) => {
            // The table exists without `allow`: the line goes under its header.
            let header = format!("[projects.{}]", crate::core::grant::toml_string(root));
            let at = text
                .match_indices(&header)
                .map(|(i, _)| i)
                .find(|&i| i == 0 || text.as_bytes()[i - 1] == b'\n')
                .with_context(|| {
                    format!(
                        "the table for {root} is not written as `{header}`, so Devplane will not \
                         guess where to add the line; add `allow = [{quoted}]` under it yourself"
                    )
                })?;
            let line_end = text[at..]
                .find('\n')
                .map(|i| at + i + 1)
                .unwrap_or(text.len());
            let prefix = if line_end == text.len() && !text.ends_with('\n') {
                "\n"
            } else {
                ""
            };
            out.insert_str(line_end, &format!("{prefix}allow = [\n  {quoted},\n]\n"));
        }
        None => {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&format!(
                "[projects.{}]\nallow = [\n  {quoted},\n]\n",
                crate::core::grant::toml_string(root)
            ));
        }
    }
    let after = granted(&out, root)?;
    if after.len() != before.len() + 1 || !after.iter().any(|r| r == rule) {
        bail!("adding the line would not have said what was meant, so the file was left as it is");
    }
    Ok(out)
}

/// The text with `rule` taken out of `[projects."<root>"] allow`; `None` when
/// the file does not grant it.
pub fn without_rule(text: &str, root: &str, rule: &str) -> Result<Option<String>> {
    let before = granted(text, root)?;
    if !before.iter().any(|r| r == rule) {
        return Ok(None);
    }
    let spans: Spans = toml::from_str(text).context("reading the file's layout")?;
    let list = spans
        .projects
        .get(root)
        .and_then(|p| p.allow.as_ref())
        .context("the grant's line could not be found")?;
    let whole = list.span();
    let el = list
        .get_ref()
        .iter()
        .find(|e| rule_of(e.get_ref()) == Some(rule))
        .context("the grant's line could not be found")?;
    let (mut start, mut end) = (el.span().start, el.span().end);
    let rest = &text[end..whole.end];
    match rest.find(|c: char| !c.is_whitespace()) {
        Some(i) if rest.as_bytes()[i] == b',' => end += i + 1,
        _ => {
            // The last entry: take the comma before it instead.
            let head = &text[whole.start + 1..start];
            if let Some(i) = head.rfind(|c: char| !c.is_whitespace())
                && head.as_bytes()[i] == b','
            {
                start = whole.start + 1 + i;
            }
        }
    }
    // A line left holding only spaces goes too.
    let line_start = text[..start].rfind('\n').map(|i| i + 1).unwrap_or(0);
    let line_end = text[end..]
        .find('\n')
        .map(|i| end + i)
        .unwrap_or(text.len());
    if text[line_start..start].trim().is_empty() && text[end..line_end].trim().is_empty() {
        start = line_start;
        end = (line_end + 1).min(text.len());
    }
    let mut out = text.to_string();
    out.replace_range(start..end, "");
    let after = granted(&out, root)?;
    if after.len() + 1 != before.len() || after.iter().any(|r| r == rule) {
        bail!(
            "removing the line would not have said what was meant, so the file was left as it is"
        );
    }
    Ok(Some(out))
}

/// Adds `rule` for `root` to the person's file.
pub fn add(home: &Path, root: &str, rule: &str) -> Result<()> {
    let text = read(home)?;
    let out = with_rule(&text, root, rule)?;
    if out != text {
        write(home, &out)?;
    }
    Ok(())
}

/// Removes `rule` for `root` from the person's file; `false` when it was not there.
pub fn remove(home: &Path, root: &str, rule: &str) -> Result<bool> {
    let text = read(home)?;
    match without_rule(&text, root, rule)? {
        Some(out) => {
            write(home, &out)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

fn write(home: &Path, body: &str) -> Result<()> {
    std::fs::create_dir_all(home).with_context(|| format!("creating {}", home.display()))?;
    let at = path(home);
    let tmp = home.join(format!(".{FILE}.{}.tmp", uuid::Uuid::new_v4().simple()));
    {
        use std::io::Write;
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts
            .open(&tmp)
            .with_context(|| format!("writing {}", tmp.display()))?;
        f.write_all(body.as_bytes())
            .and_then(|_| f.sync_all())
            .with_context(|| format!("writing {}", tmp.display()))?;
    }
    std::fs::rename(&tmp, &at).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        anyhow::anyhow!("writing {}: {e}", at.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const FILE_TEXT: &str = "# my rules\n[policy]\ndeny = [\"Bash(rm -rf *)\"] # never\n";

    #[test]
    fn adding_keeps_every_comment_and_line() {
        let once = with_rule(FILE_TEXT, "/r", "Bash(cargo test)").unwrap();
        assert!(once.starts_with(FILE_TEXT), "{once}");
        assert_eq!(granted(&once, "/r").unwrap(), ["Bash(cargo test)"]);
        let twice = with_rule(&once, "/r", "Bash(cargo fmt)").unwrap();
        assert!(twice.contains("# my rules") && twice.contains("# never"));
        assert_eq!(
            granted(&twice, "/r").unwrap(),
            ["Bash(cargo test)", "Bash(cargo fmt)"]
        );
        assert_eq!(with_rule(&twice, "/r", "Bash(cargo fmt)").unwrap(), twice);

        // A one-line array, with and without a trailing comma, and an empty one.
        for (text, n) in [
            ("[projects.\"/r\"]\nallow = [\"Bash(ls)\"]\n", 2),
            ("[projects.\"/r\"]\nallow = [\"Bash(ls)\",] # mine\n", 2),
            ("[projects.\"/r\"]\nallow = []\n", 1),
            ("[projects.\"/r\"]\noffer_after = 3\n", 1),
        ] {
            let out = with_rule(text, "/r", "Bash(cargo test)").unwrap();
            assert_eq!(granted(&out, "/r").unwrap().len(), n, "{out}");
            assert!(!text.contains("# mine") || out.contains("# mine"));
        }
    }

    #[test]
    fn removing_takes_only_that_entry() {
        let text = "# keep\n[projects.\"/r\"]\nallow = [\n  \"Bash(cargo test)\", # first\n  { rule = \"Bash(cargo fmt)\", until = \"2026-12-01\" },\n  \"Bash(ls)\",\n]\n";
        let out = without_rule(text, "/r", "Bash(cargo fmt)")
            .unwrap()
            .unwrap();
        assert_eq!(
            granted(&out, "/r").unwrap(),
            ["Bash(cargo test)", "Bash(ls)"]
        );
        assert!(out.contains("# keep") && out.contains("# first"), "{out}");
        let out = without_rule(&out, "/r", "Bash(ls)").unwrap().unwrap();
        assert_eq!(granted(&out, "/r").unwrap(), ["Bash(cargo test)"]);
        let one = "[projects.\"/r\"]\nallow = [\"Bash(a)\", \"Bash(b)\"]\n";
        let out = without_rule(one, "/r", "Bash(b)").unwrap().unwrap();
        assert_eq!(granted(&out, "/r").unwrap(), ["Bash(a)"]);
        assert_eq!(without_rule(one, "/r", "Bash(zzz)").unwrap(), None);
        assert_eq!(without_rule(one, "/other", "Bash(a)").unwrap(), None);
    }

    #[test]
    fn a_broken_file_is_never_overwritten() {
        let home = std::env::temp_dir().join(format!("dp-gf-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(path(&home), "[policy\n").unwrap();
        assert!(add(&home, "/r", "Bash(ls)").is_err());
        assert_eq!(std::fs::read_to_string(path(&home)).unwrap(), "[policy\n");
        std::fs::remove_file(path(&home)).unwrap();
        add(&home, "/r", "Bash(ls)").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path(&home)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "the file is owner-only");
        }
        assert!(remove(&home, "/r", "Bash(ls)").unwrap());
        assert!(!remove(&home, "/r", "Bash(ls)").unwrap());
    }
}
