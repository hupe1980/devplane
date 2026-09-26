//! Projects — a registered repository root and its configuration.

use crate::core::ids::ProjectId;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// A registered repository. It must be trusted before Devplane spawns an agent
/// in it: a headless run executes the repository's own hooks and MCP servers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: String,
    pub root: PathBuf,
    /// Trusted projects may host driven and background runs; untrusted ones
    /// are observe-only.
    pub trusted: bool,
    /// Correlates OpenTelemetry's `vcs.repository.url.full` without a path lookup.
    pub repo_url: Option<String>,
    /// Discovered rather than registered: a session appeared in this directory.
    pub auto_discovered: bool,
}

impl Project {
    /// The `owner/name` slug a `claude-cli://open?repo=` link needs, from any
    /// remote form [`parse_remote`] reads.
    pub fn repo_slug(&self) -> Option<String> {
        let r = parse_remote(self.repo_url.as_deref()?)?;
        Some(format!("{}/{}", r.owner, r.name))
    }

    pub fn from_root(root: PathBuf) -> Self {
        let name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| root.to_string_lossy().to_string());
        Self {
            id: ProjectId::from_path(&root),
            name,
            root,
            trusted: false,
            repo_url: None,
            auto_discovered: false,
        }
    }

    /// Whether a path belongs to this project, including worktrees under
    /// `.claude/worktrees/`.
    pub fn contains(&self, path: &Path) -> bool {
        path.starts_with(&self.root)
    }
}

/// A git remote read as the repository it names: `(host, owner, name)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Remote {
    /// Lower-cased, without a port: one server answers HTTPS and SSH on
    /// different ports, and both remotes name the same host. GitHub's
    /// SSH-over-443 name, `ssh.github.com`, is `github.com`.
    pub host: String,
    pub owner: String,
    pub name: String,
}

/// Reads `https://host/owner/name(.git)`, `ssh://git@host[:port]/owner/name`
/// and `git@host:owner/name(.git)`. Anything with a deeper path is not an
/// `owner/name` repository, and guessing would open the wrong one.
pub fn parse_remote(url: &str) -> Option<Remote> {
    let url = url.trim().trim_end_matches('/');
    let (host, path) = match url.split_once("://") {
        Some((_scheme, after)) => {
            let (authority, path) = after.split_once('/')?;
            // `user@host` — the user is the SSH or HTTP login, never the host.
            let authority = authority.rsplit('@').next()?;
            (authority.split(':').next()?, path)
        }
        None => {
            // scp-like: `[user@]host:owner/name`. A Windows drive (`C:\…`) or
            // a local path is not a remote.
            let (authority, path) = url.split_once(':')?;
            if authority.contains('/') || authority.contains('\\') || authority.len() < 2 {
                return None;
            }
            (authority.rsplit('@').next()?, path)
        }
    };
    let path = path.trim_start_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, name) = path.split_once('/')?;
    let host = match host.to_ascii_lowercase() {
        h if h == "ssh.github.com" => "github.com".to_string(),
        h => h,
    };
    (!host.is_empty() && !owner.is_empty() && !name.is_empty() && !name.contains('/')).then(|| {
        Remote {
            host,
            owner: owner.to_string(),
            name: name.to_string(),
        }
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_repo_slug_is_read_from_either_remote_form() {
        let mut p = Project::from_root(PathBuf::from("/repo"));
        assert_eq!(p.repo_slug(), None, "no remote, no link");

        p.repo_url = Some("https://github.com/acme/payments".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("https://github.com/acme/payments.git".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("git@github.com:acme/payments.git".into());
        assert_eq!(p.repo_slug().as_deref(), Some("acme/payments"));

        p.repo_url = Some("https://gitlab.example.com/group/sub/app.git".into());
        assert_eq!(
            p.repo_slug(),
            None,
            "a nested path is not a slug, and guessing would open the wrong repository"
        );
    }

    #[test]
    fn every_remote_form_names_host_owner_and_name() {
        let r = |host: &str| Remote {
            host: host.into(),
            owner: "acme".into(),
            name: "app".into(),
        };
        for url in [
            "https://github.com/acme/app",
            "https://github.com/acme/app.git",
            "https://github.com/acme/app/",
            "https://someone@github.com/acme/app.git",
            "git@github.com:acme/app.git",
            "git@github.com:acme/app",
            "ssh://git@github.com/acme/app.git",
            "ssh://git@github.com:22/acme/app",
            "git@GitHub.com:acme/app.git",
            "ssh://git@ssh.github.com:443/acme/app.git",
            "git@ssh.github.com:acme/app.git",
        ] {
            assert_eq!(parse_remote(url), Some(r("github.com")), "{url}");
        }
        for url in [
            "https://ghe.corp:8443/acme/app.git",
            "ssh://git@ghe.corp:2222/acme/app.git",
            "git@ghe.corp:acme/app.git",
        ] {
            assert_eq!(
                parse_remote(url),
                Some(r("ghe.corp")),
                "one host whichever port its remote names: {url}"
            );
        }
        assert_eq!(
            parse_remote("git@gitlab.example.com:acme/app.git"),
            Some(r("gitlab.example.com")),
            "a non-GitHub host still parses; whether it is GitHub is the caller's question"
        );
        for not in [
            "",
            "/srv/git/app.git",
            "C:\\repos\\app",
            "https://gitlab.example.com/group/sub/app.git",
            "https://github.com/acme",
        ] {
            assert_eq!(parse_remote(not), None, "{not}");
        }
    }

    use super::*;
}
