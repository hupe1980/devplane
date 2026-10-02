//! Which repositories a person trusted: `~/.devplane/trusted.toml`.
//!
//! A preference, so it lives in a file named for it and never in the store:
//! deleting the store loses history, never a person's decision to let agents
//! start in a repository. Written only by a person's act — `devplane project
//! trust` in their own terminal, or the window's proved route — and read by
//! the host when it starts and when it is told to re-read.
//!
//! ```toml
//! roots = ["/Users/me/code/api"]
//! ```

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// The file's name under Devplane's home.
pub const FILE: &str = "trusted.toml";

/// What the file says. Unknown keys are refused, as every Devplane file
/// refuses them: a misspelt key must not read as *nothing trusted* silently.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Trusted {
    /// Canonical repository roots, in order.
    pub roots: BTreeSet<PathBuf>,
}

/// `<home>/trusted.toml`.
pub fn path(home: &Path) -> PathBuf {
    home.join(FILE)
}

impl Trusted {
    /// Reads the file. Missing means nothing is trusted; unreadable or
    /// malformed is an error, never an empty list, so a caller decides what a
    /// broken file means rather than this quietly revoking every trust.
    pub fn read(home: &Path) -> Result<Self> {
        let at = path(home);
        let text = match std::fs::read_to_string(&at) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(e).with_context(|| format!("reading {}", at.display())),
        };
        toml::from_str(&text).with_context(|| format!("reading {}", at.display()))
    }

    /// Whether `root` is trusted.
    pub fn contains(&self, root: &Path) -> bool {
        self.roots.contains(root)
    }

    /// The trusted roots as projects, each marked trusted, for a world to
    /// merge in. A root the store never recorded is registered by it.
    pub fn projects(&self) -> impl Iterator<Item = crate::core::Project> + '_ {
        self.roots.iter().map(|root| {
            let mut p = crate::core::Project::from_root(root.clone());
            p.trusted = true;
            p
        })
    }
}

/// Adds `roots` to the file, keeping what it already says. Written whole to
/// a sibling and renamed into place, owner-only, so a reader never sees half
/// a file and a crash never loses an earlier trust. A file that cannot be
/// read is not overwritten: the error says so.
pub fn add(home: &Path, roots: impl IntoIterator<Item = PathBuf>) -> Result<Trusted> {
    let mut t = Trusted::read(home)?;
    let before = t.roots.len();
    t.roots.extend(roots);
    if t.roots.len() == before && path(home).exists() {
        return Ok(t);
    }
    write(home, &t)?;
    Ok(t)
}

fn write(home: &Path, t: &Trusted) -> Result<()> {
    std::fs::create_dir_all(home).with_context(|| format!("creating {}", home.display()))?;
    let at = path(home);
    let body = format!(
        "# Repositories a person trusted Devplane to start agents in. Written by\n\
         # `devplane project trust` and by the window; a root removed here is no\n\
         # longer trusted once the host restarts.\n\n{}",
        toml::to_string(t).context("writing the trusted roots")?
    );
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

    fn home() -> PathBuf {
        let h = std::env::temp_dir().join(format!("dp-trust-{}", uuid::Uuid::new_v4().simple()));
        std::fs::create_dir_all(&h).unwrap();
        h
    }

    #[test]
    fn a_missing_file_trusts_nothing() {
        assert_eq!(Trusted::read(&home()).unwrap(), Trusted::default());
    }

    #[test]
    fn trust_is_added_and_kept() {
        let h = home();
        add(&h, [PathBuf::from("/r/a")]).unwrap();
        add(&h, [PathBuf::from("/r/b")]).unwrap();
        let t = Trusted::read(&h).unwrap();
        assert!(t.contains(Path::new("/r/a")) && t.contains(Path::new("/r/b")));
        assert!(t.projects().all(|p| p.trusted));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(path(&h)).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "the file is owner-only");
        }
    }

    #[test]
    fn a_misspelt_or_broken_file_is_an_error_and_is_never_overwritten() {
        let h = home();
        std::fs::write(path(&h), "root = [\"/r/a\"]\n").unwrap();
        let e = Trusted::read(&h).unwrap_err();
        assert!(format!("{e:#}").contains("unknown field"), "{e:#}");
        assert!(add(&h, [PathBuf::from("/r/b")]).is_err());
        assert_eq!(
            std::fs::read_to_string(path(&h)).unwrap(),
            "root = [\"/r/a\"]\n",
            "a file a person may have meant is left as it is"
        );
    }
}
