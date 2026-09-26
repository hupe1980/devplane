//! Signing in to GitHub by the OAuth device flow, and where the token lives.
//!
//! The token is kept only in the operating system's credential store
//! (service `devplane`, account `github:<host>`). Where there is none, sign-in
//! refuses; there is no file fallback.

use super::Error;
use super::client::GitHubHost;
use jiff::Timestamp;
use serde::Deserialize;

/// The sentence a login without a client id refuses with: the device flow
/// needs an OAuth App registered on the host, and a token signs in without
/// one.
pub const NO_CLIENT_ID: &str = "no GitHub app is registered for this host, so the device flow \
cannot start. Sign in with a token instead: `gh auth token | devplane login github --with-token`, \
or pipe a fine-grained personal access token to `devplane login github --with-token`. An \
Enterprise server's own app goes in ~/.devplane/app.toml as `[github.hosts.\"<host>\"] client_id`";

/// The scopes asked for: `repo` to read and open pull requests and issues on
/// private repositories, `read:org` so review requests to a team are seen.
pub const SCOPES: &str = "repo read:org";

/// A token, or a device code: redacted wherever it is printed.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The value, for the one place it goes: a header to its own host, or
    /// the credential store.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl std::fmt::Display for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[redacted]")
    }
}

/// Where tokens are kept, keyed by GitHub host.
pub trait TokenStore: Send + Sync {
    fn get(&self, host: &str) -> Result<Option<Secret>, Error>;
    fn set(&self, host: &str, token: &Secret) -> Result<(), Error>;
    fn delete(&self, host: &str) -> Result<(), Error>;
    /// Where a token for `host` is, in words: `the macOS Keychain (service
    /// devplane, account github:github.com)`.
    fn describe(&self, host: &str) -> String;
}

/// The credential-store account for a host.
pub fn account(host: &str) -> String {
    format!("github:{host}")
}

/// The service name every entry is filed under.
pub const SERVICE: &str = "devplane";

/// The operating system's credential store.
#[derive(Debug)]
pub struct Keyring {
    _private: (),
}

impl Keyring {
    /// The store, or why there is none. Probes it once, so a Linux without a
    /// Secret Service is said here rather than on the first write.
    pub fn open() -> Result<Self, Error> {
        if !cfg!(any(
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "linux",
            target_os = "freebsd",
            target_os = "openbsd"
        )) {
            return Err(Error::NoCredentialStore(
                "this platform has no credential store Devplane can use".into(),
            ));
        }
        let probe = keyring::Entry::new(SERVICE, &account("probe")).map_err(no_store)?;
        match probe.get_password() {
            Ok(_) | Err(keyring::Error::NoEntry) => Ok(Self { _private: () }),
            Err(e) => Err(no_store(e)),
        }
    }

    fn entry(host: &str) -> Result<keyring::Entry, Error> {
        keyring::Entry::new(SERVICE, &account(host)).map_err(no_store)
    }
}

fn no_store(e: keyring::Error) -> Error {
    match e {
        keyring::Error::NoStorageAccess(_) | keyring::Error::PlatformFailure(_) => {
            Error::NoCredentialStore(e.to_string())
        }
        other => Error::Store(other.to_string()),
    }
}

/// The store's own name on this platform.
fn platform_store() -> &'static str {
    if cfg!(target_os = "macos") {
        "the macOS Keychain"
    } else if cfg!(target_os = "windows") {
        "the Windows Credential Manager"
    } else {
        "the Secret Service"
    }
}

impl TokenStore for Keyring {
    fn get(&self, host: &str) -> Result<Option<Secret>, Error> {
        match Self::entry(host)?.get_password() {
            Ok(t) => Ok(Some(Secret::new(t))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(no_store(e)),
        }
    }

    fn set(&self, host: &str, token: &Secret) -> Result<(), Error> {
        Self::entry(host)?
            .set_password(token.expose())
            .map_err(no_store)
    }

    fn delete(&self, host: &str) -> Result<(), Error> {
        match Self::entry(host)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(no_store(e)),
        }
    }

    fn describe(&self, host: &str) -> String {
        format!(
            "{} (service {SERVICE}, account {})",
            platform_store(),
            account(host)
        )
    }
}

/// An in-memory store, for tests and nothing else: it forgets on exit.
#[derive(Debug, Default)]
pub struct Memory {
    tokens: std::sync::Mutex<std::collections::BTreeMap<String, Secret>>,
}

impl Memory {
    /// Every host with a token, for a test to assert on.
    pub fn hosts(&self) -> Vec<String> {
        self.tokens
            .lock()
            .map(|t| t.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Whether any stored value contains `needle`.
    pub fn contains(&self, needle: &str) -> bool {
        self.tokens
            .lock()
            .map(|t| t.values().any(|s| s.expose().contains(needle)))
            .unwrap_or(false)
    }
}

impl TokenStore for Memory {
    fn get(&self, host: &str) -> Result<Option<Secret>, Error> {
        Ok(self
            .tokens
            .lock()
            .map_err(|_| Error::Store("poisoned".into()))?
            .get(host)
            .cloned())
    }
    fn set(&self, host: &str, token: &Secret) -> Result<(), Error> {
        self.tokens
            .lock()
            .map_err(|_| Error::Store("poisoned".into()))?
            .insert(host.to_string(), token.clone());
        Ok(())
    }
    fn delete(&self, host: &str) -> Result<(), Error> {
        self.tokens
            .lock()
            .map_err(|_| Error::Store("poisoned".into()))?
            .remove(host);
        Ok(())
    }
    fn describe(&self, host: &str) -> String {
        format!("memory (account {})", account(host))
    }
}

/// A device-flow sign-in waiting for the person to enter the code.
#[derive(Debug, Clone)]
pub struct Pending {
    pub host: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: Timestamp,
    /// Seconds between polls; raised by `slow_down`.
    pub interval: u64,
    device_code: Secret,
}

/// What one poll said.
#[derive(Debug, Clone, PartialEq)]
pub enum Poll {
    Waiting,
    /// Poll less often: the new interval, in seconds.
    SlowDown(u64),
    Done {
        token: Secret,
        scopes: Vec<String>,
    },
    Denied,
    Expired,
}

fn form_post(url: &str) -> reqwest::RequestBuilder {
    super::client::http()
        .post(url)
        .header(reqwest::header::ACCEPT, "application/json")
}

/// Starts the device flow: a code for the person to enter at GitHub.
pub async fn start(host: &GitHubHost, client_id: Option<&str>) -> Result<Pending, Error> {
    let client_id = client_id.ok_or(Error::NoClientId)?;
    #[derive(Deserialize)]
    struct Code {
        device_code: String,
        user_code: String,
        verification_uri: String,
        expires_in: i64,
        #[serde(default)]
        interval: Option<u64>,
    }
    let resp = form_post(&format!("{}/login/device/code", host.oauth))
        .form(&[("client_id", client_id), ("scope", SCOPES)])
        .send()
        .await
        .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
    let status = resp.status();
    let text = resp
        .text()
        .await
        .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
    let code: Code = serde_json::from_str(&text).map_err(|_| Error::Api {
        status: status.as_u16(),
        message: device_error(&text),
    })?;
    Ok(Pending {
        host: host.name.clone(),
        user_code: code.user_code,
        verification_uri: code.verification_uri,
        expires_at: Timestamp::now() + jiff::SignedDuration::from_secs(code.expires_in.max(1)),
        interval: code.interval.unwrap_or(5).max(1),
        device_code: Secret::new(code.device_code),
    })
}

fn device_error(text: &str) -> String {
    let v: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    v["error_description"]
        .as_str()
        .or(v["error"].as_str())
        .unwrap_or("GitHub would not start a sign-in")
        .to_string()
}

/// Asks once whether the person has entered the code.
pub async fn poll(host: &GitHubHost, client_id: Option<&str>, p: &Pending) -> Result<Poll, Error> {
    let client_id = client_id.ok_or(Error::NoClientId)?;
    if Timestamp::now() >= p.expires_at {
        return Ok(Poll::Expired);
    }
    let resp = form_post(&format!("{}/login/oauth/access_token", host.oauth))
        .form(&[
            ("client_id", client_id),
            ("device_code", p.device_code.expose()),
            ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
        ])
        .send()
        .await
        .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
    let v: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| Error::Unreachable(e.without_url().to_string()))?;
    if let Some(token) = v["access_token"].as_str().filter(|t| !t.is_empty()) {
        let scopes = v["scope"]
            .as_str()
            .unwrap_or_default()
            .split([',', ' '])
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
        return Ok(Poll::Done {
            token: Secret::new(token),
            scopes,
        });
    }
    Ok(match v["error"].as_str().unwrap_or_default() {
        "authorization_pending" => Poll::Waiting,
        "slow_down" => Poll::SlowDown(
            v["interval"]
                .as_u64()
                .unwrap_or(p.interval + 5)
                .max(p.interval + 1),
        ),
        "access_denied" => Poll::Denied,
        "expired_token" | "token_expired" => Poll::Expired,
        other => {
            return Err(Error::Api {
                status: 400,
                message: v["error_description"]
                    .as_str()
                    .unwrap_or(if other.is_empty() {
                        "GitHub answered the sign-in poll with nothing"
                    } else {
                        other
                    })
                    .to_string(),
            });
        }
    })
}

/// Who a token signs in as, and what it may do.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Viewer {
    pub login: String,
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Asks GitHub whom a token belongs to (`GET /user`).
pub async fn verify(host: &GitHubHost, token: &Secret) -> Result<Viewer, Error> {
    let c = super::client::Client::new(host.clone(), token.clone());
    let a = c.rest(reqwest::Method::GET, "/user", None).await?;
    let login = a.body["login"]
        .as_str()
        .filter(|l| !l.is_empty())
        .ok_or_else(|| Error::Api {
            status: 200,
            message: "GitHub named no login for this token".into(),
        })?
        .to_string();
    Ok(Viewer {
        login,
        scopes: a.scopes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints() {
        let s = Secret::new("gho_abc123");
        assert_eq!(format!("{s}"), "[redacted]");
        assert!(!format!("{s:?}").contains("gho_abc123"));
        assert_eq!(s.expose(), "gho_abc123");
    }

    #[test]
    fn the_memory_store_keeps_one_token_per_host() {
        let m = Memory::default();
        m.set("github.com", &Secret::new("a")).unwrap();
        m.set("ghe.corp", &Secret::new("b")).unwrap();
        assert_eq!(m.get("github.com").unwrap(), Some(Secret::new("a")));
        m.delete("github.com").unwrap();
        assert_eq!(m.get("github.com").unwrap(), None);
        assert_eq!(m.hosts(), ["ghe.corp"]);
        assert_eq!(account("ghe.corp"), "github:ghe.corp");
    }

    #[tokio::test]
    async fn no_client_id_refuses_with_the_documented_sentence() {
        let e = start(&GitHubHost::of("github.com"), None)
            .await
            .unwrap_err();
        assert_eq!(e.to_string(), NO_CLIENT_ID);
    }
}
