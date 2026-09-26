//! A working directory's git remote, read as the GitHub repository it names.
//!
//! The parser is `core::project::parse_remote` (pure, shared with the launch
//! links); this side asks git, which applies `insteadOf` rewrites itself.

use std::path::Path;

/// `(host, owner, name)` on a GitHub host.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RepoRef {
    pub host: String,
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    /// `owner/name`, the form GitHub's search and URLs use.
    pub fn slug(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }

    /// Reads a remote URL. `None` when it names no `owner/name` repository.
    pub fn parse(url: &str) -> Option<Self> {
        let r = crate::core::project::parse_remote(url)?;
        Some(Self {
            host: r.host,
            owner: r.owner,
            name: r.name,
        })
    }

    /// GitHub's own "open a pull request" page for a branch, prefilled. The
    /// branch names are percent-encoded, keeping `/`: a `#` or `?` in one
    /// would otherwise end the path.
    pub fn compare_url(&self, base: &str, branch: &str) -> String {
        format!(
            "https://{}/{}/{}/compare/{}...{}?expand=1",
            self.host,
            self.owner,
            self.name,
            encode_ref(base),
            encode_ref(branch)
        )
    }
}

/// A branch name as a URL path segment: unreserved characters and `/` kept,
/// everything else percent-encoded byte by byte.
fn encode_ref(r: &str) -> String {
    let mut out = String::with_capacity(r.len());
    for b in r.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The repository a directory's remote names: `origin`, else the first remote.
///
/// `Err` says why there is none — no remote, or one that names no
/// `owner/name` repository — in words a Forge row can show.
pub async fn of_dir(dir: &Path) -> Result<RepoRef, super::Error> {
    let url = crate::git::forge_remote_url(dir)
        .await
        .ok_or_else(|| super::Error::NotGitHub("no git remote".into()))?;
    RepoRef::parse(&url).ok_or_else(|| {
        super::Error::NotGitHub(format!("the remote `{url}` names no owner/name repository"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_compare_url_is_githubs_own_page_for_the_branch() {
        let r = RepoRef::parse("git@github.com:acme/app.git").unwrap();
        assert_eq!(r.slug(), "acme/app");
        assert_eq!(
            r.compare_url("main", "feat/x"),
            "https://github.com/acme/app/compare/main...feat/x?expand=1"
        );
        assert_eq!(
            r.compare_url("release/1.0", "fix/a#b?c d"),
            "https://github.com/acme/app/compare/release/1.0...fix/a%23b%3Fc%20d?expand=1",
            "a branch cannot end the path early"
        );
        let e = RepoRef::parse("https://ghe.corp/acme/app").unwrap();
        assert_eq!(e.host, "ghe.corp");
    }

    fn repo(tag: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!(
            "dp-remote-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&d).unwrap();
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .args(args)
                .current_dir(&d)
                .output()
                .unwrap()
                .status
                .success();
            assert!(ok, "git {args:?}");
        };
        git(&["init", "-q"]);
        d
    }

    #[tokio::test]
    async fn the_remote_is_read_through_git_with_its_rewrites() {
        let d = repo("insteadof");
        let run = |args: &[&str]| {
            std::process::Command::new("git")
                .args(args)
                .current_dir(&d)
                .output()
                .unwrap();
        };
        assert!(matches!(
            of_dir(&d).await,
            Err(super::super::Error::NotGitHub(_))
        ));
        // Not `origin`: the first remote is used.
        run(&["remote", "add", "upstream", "short:acme/app"]);
        run(&["config", "url.git@github.com:.insteadOf", "short:"]);
        let r = of_dir(&d).await.unwrap();
        assert_eq!(
            (r.host.as_str(), r.slug().as_str()),
            ("github.com", "acme/app")
        );
        // `origin` wins once there is one.
        run(&["remote", "add", "origin", "https://ghe.corp/team/tool.git"]);
        let r = of_dir(&d).await.unwrap();
        assert_eq!(
            (r.host.as_str(), r.slug().as_str()),
            ("ghe.corp", "team/tool")
        );
        std::fs::remove_dir_all(&d).ok();
    }
}
