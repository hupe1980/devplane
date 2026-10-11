//! The person's signature on an attested statement: `ssh-keygen -Y sign` with
//! the key the person's **global** git configuration names, on their request
//! at export, in the CLI's own process. Devplane never makes a key, never
//! stores one, and contacts nothing.
//!
//! Only the global configuration is read. A worktree's `.git/config` is
//! writable by the agent working in it, so a repository-local
//! `user.signingkey` would let it choose which key signs, and
//! `gpg.ssh.program` or `gpg.ssh.defaultKeyCommand` would let it run a command
//! here; neither is ever read, and `ssh-keygen` is found on `PATH`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::core::attest::{Recipe, SIGNING_NAMESPACE};

/// Why nothing was signed. Each says so in the person's terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SignError {
    NoSigningKey,
    KeyUnreadable(PathBuf),
    SshKeygenMissing,
    SshKeygenFailed(String),
    OutExists(PathBuf),
    Write(String),
}

impl std::fmt::Display for SignError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SignError::NoSigningKey => write!(
                f,
                "no SSH signing key: set one with `git config --global user.signingkey \
                 <path-or-key::…>`. Devplane never makes a key."
            ),
            SignError::KeyUnreadable(p) => {
                write!(f, "the signing key `{}` does not exist", p.display())
            }
            SignError::SshKeygenMissing => {
                write!(f, "`ssh-keygen` was not found on PATH; it is what signs")
            }
            SignError::SshKeygenFailed(why) => write!(f, "ssh-keygen could not sign: {why}"),
            SignError::OutExists(p) => {
                write!(f, "{} exists; pass --force to replace it", p.display())
            }
            SignError::Write(why) => write!(f, "{why}"),
        }
    }
}

impl std::error::Error for SignError {}

/// Where the signing key is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Key {
    /// A key file: the private half, or a public one whose private half the
    /// SSH agent holds.
    Path(PathBuf),
    /// `key::<public key>`: the private half is in the SSH agent.
    Literal(String),
}

/// The key the person named, and the principal a reader checks it under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SigningKey {
    pub key: Key,
    /// The person's global `user.email`, or `signer`.
    pub identity: String,
}

/// One value from the **global** git configuration (`GIT_CONFIG_GLOBAL`
/// honoured), never the repository's.
fn global(name: &str) -> Option<String> {
    let out = Command::new("git")
        .args(["config", "--global", "--get", name])
        .env_remove("GIT_CONFIG_PARAMETERS")
        .env_remove("GIT_CONFIG_COUNT")
        .stdin(Stdio::null())
        .output()
        .ok()?;
    let v = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (out.status.success() && !v.is_empty()).then_some(v)
}

fn expand_home(p: &str) -> PathBuf {
    match p.strip_prefix("~/") {
        Some(rest) => dirs_home()
            .map(|h| h.join(rest))
            .unwrap_or_else(|| p.into()),
        None => PathBuf::from(p),
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

/// A principal an allowed-signers line can carry as written.
fn principal(s: &str) -> Option<String> {
    let ok = !s.is_empty()
        && s.chars()
            .all(|c| !c.is_whitespace() && !c.is_control() && !matches!(c, ',' | '"' | '\''));
    ok.then(|| s.to_string())
}

impl SigningKey {
    /// The person's global `user.signingkey` and `user.email`.
    pub fn from_global() -> Result<Self, SignError> {
        let raw = global("user.signingkey").ok_or(SignError::NoSigningKey)?;
        let key = match raw.strip_prefix("key::") {
            Some(literal) => Key::Literal(literal.trim().to_string()),
            None => {
                let p = expand_home(&raw);
                if !p.is_file() {
                    return Err(SignError::KeyUnreadable(p));
                }
                Key::Path(p)
            }
        };
        let identity = global("user.email")
            .as_deref()
            .and_then(principal)
            .unwrap_or_else(|| "signer".into());
        Ok(Self { key, identity })
    }

    /// `<keytype> <base64>` of the public half.
    fn public_key(&self) -> Result<String, SignError> {
        let text = match &self.key {
            Key::Literal(l) => l.clone(),
            Key::Path(p) => {
                let public = match p.extension().is_some_and(|e| e == "pub") {
                    true => p.clone(),
                    false => PathBuf::from(format!("{}.pub", p.display())),
                };
                match std::fs::read_to_string(&public) {
                    Ok(t) => t,
                    // No `.pub` beside the private key: ssh-keygen derives it.
                    Err(_) => ssh_keygen(&["-y", "-f", &p.to_string_lossy()], None)?,
                }
            }
        };
        let mut parts = text.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some(t), Some(b)) => Ok(format!("{t} {b}")),
            _ => Err(SignError::SshKeygenFailed(
                "the signing key's public half could not be read".into(),
            )),
        }
    }
}

/// Runs `ssh-keygen` from `PATH`, with the terminal for a passphrase prompt.
fn ssh_keygen(args: &[&str], dir: Option<&Path>) -> Result<String, SignError> {
    let mut cmd = Command::new("ssh-keygen");
    cmd.args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(d) = dir {
        cmd.current_dir(d);
    }
    let out = cmd.output().map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => SignError::SshKeygenMissing,
        _ => SignError::SshKeygenFailed(e.to_string()),
    })?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err
            .lines()
            .map(str::trim)
            .rfind(|l| !l.is_empty())
            .unwrap_or("it exited without saying why")
            .to_string();
        return Err(SignError::SshKeygenFailed(line));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The key's `SHA256:` fingerprint, for the line that says what signed.
pub fn fingerprint(public_key: &str, scratch: &Path) -> Option<String> {
    let f = scratch.join(".devplane-fingerprint.pub");
    std::fs::write(&f, format!("{public_key}\n")).ok()?;
    let out = ssh_keygen(&["-l", "-f", &f.to_string_lossy()], None).ok();
    std::fs::remove_file(&f).ok();
    out?.split_whitespace().nth(1).map(str::to_string)
}

/// A file removed when dropped, unless kept.
struct Temp(Option<PathBuf>);

impl Temp {
    fn keep(&mut self) {
        self.0 = None;
    }
}

impl Drop for Temp {
    fn drop(&mut self) {
        if let Some(p) = &self.0 {
            std::fs::remove_file(p).ok();
        }
    }
}

/// Writes `bytes` to `out` and a detached SSH signature over exactly them to
/// `out.sig` — both or neither. The signature is made on a sibling file and
/// both are moved into place only once it exists, so a refusal or a failure
/// leaves whatever was there before untouched. An existing `out` or
/// `out.sig` is refused unless `force`.
pub fn sign_to(
    out: &Path,
    bytes: &[u8],
    key: &SigningKey,
    force: bool,
) -> Result<Recipe, SignError> {
    let sig = PathBuf::from(format!("{}.sig", out.display()));
    if !force {
        for p in [out, sig.as_path()] {
            if p.exists() {
                return Err(SignError::OutExists(p.to_path_buf()));
            }
        }
    }
    let dir = match out.parent() {
        Some(d) if !d.as_os_str().is_empty() => d.to_path_buf(),
        _ => PathBuf::from("."),
    };
    let file = out
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| SignError::Write(format!("{} names no file", out.display())))?;
    let public_key = key.public_key()?;

    let staged = dir.join(format!(".{file}.devplane-{}", std::process::id()));
    let staged_sig = PathBuf::from(format!("{}.sig", staged.display()));
    let mut t_body = Temp(Some(staged.clone()));
    let mut t_sig = Temp(Some(staged_sig.clone()));
    std::fs::remove_file(&staged_sig).ok();
    std::fs::write(&staged, bytes)
        .map_err(|e| SignError::Write(format!("{} could not be written: {e}", out.display())))?;

    // A literal key goes to a user-only file for `-f`; its private half is
    // the agent's.
    let mut t_key = Temp(None);
    let key_file = match &key.key {
        Key::Path(p) => p.clone(),
        Key::Literal(l) => {
            let p = dir.join(format!(".devplane-signing-key-{}.pub", std::process::id()));
            write_private(&p, &format!("{l}\n"))
                .map_err(|e| SignError::Write(format!("the key could not be staged: {e}")))?;
            t_key.0 = Some(p.clone());
            p
        }
    };
    ssh_keygen(
        &[
            "-Y",
            "sign",
            "-f",
            &key_file.to_string_lossy(),
            "-n",
            SIGNING_NAMESPACE,
            &staged.to_string_lossy(),
        ],
        None,
    )?;
    if !staged_sig.is_file() {
        return Err(SignError::SshKeygenFailed(
            "it exited 0 and wrote no signature".into(),
        ));
    }
    std::fs::rename(&staged_sig, &sig)
        .map_err(|e| SignError::Write(format!("{} could not be written: {e}", sig.display())))?;
    t_sig.keep();
    if let Err(e) = std::fs::rename(&staged, out) {
        std::fs::remove_file(&sig).ok();
        return Err(SignError::Write(format!(
            "{} could not be written: {e}",
            out.display()
        )));
    }
    t_body.keep();
    Ok(Recipe {
        identity: key.identity.clone(),
        public_key,
        file,
    })
}

#[cfg(unix)]
fn write_private(p: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(p)?;
    f.write_all(text.as_bytes())
}

#[cfg(not(unix))]
fn write_private(p: &Path, text: &str) -> std::io::Result<()> {
    std::fs::write(p, text)
}
