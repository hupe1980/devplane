//! The CLI's view of the host: every terminal command is a client of the same
//! API the browser uses, so anything a person can see they can script with
//! `--json`.

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;

/// The one sentence for *nothing is running*, shared by every command that
/// needs a host.
pub const NO_HOST: &str =
    "no host is running — start one with `devplane serve`, or `devplane open` for the workbench";

#[derive(Clone)]
pub struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
}

/// What a failed request means, in a sentence a person can act on.
///
/// A 404 almost always means the host is a different build of the same
/// version (unreleased trees change routes without a version bump), not that
/// the thing asked for is missing.
fn explain_status(path: &str, status: reqwest::StatusCode) -> String {
    match status {
        reqwest::StatusCode::NOT_FOUND => format!(
            "{path} returned 404 — this binary knows that route, so the running host is \
             probably an older build of it. `devplane quit` and run the command again."
        ),
        reqwest::StatusCode::UNAUTHORIZED => format!(
            "{path} returned 401 — the token in ~/.devplane/token is not the one the running \
             host started with. `devplane quit` and run the command again."
        ),
        other => format!("{path} returned {other}"),
    }
}

impl Client {
    /// Connects to the host the record names, without asking whether it answers.
    pub fn connect() -> Result<Self> {
        let info = crate::config::read_host()?.context(NO_HOST)?;
        Ok(Self {
            base: info.base_url(),
            token: crate::config::load_or_create_token()?,
            http: reqwest::Client::new(),
        })
    }

    /// Connects to a running host, or says there is none. Nothing is started
    /// by asking: the error names the command that starts one.
    pub async fn connect_running() -> Result<Self> {
        let c = Self::connect()?;
        match c.version().await {
            Some(v) if v == env!("CARGO_PKG_VERSION") => Ok(c),
            Some(v) => bail!(
                "the running host is v{v} and this is v{}. `devplane quit`, then start it again.",
                env!("CARGO_PKG_VERSION")
            ),
            None => bail!("{NO_HOST}"),
        }
    }

    /// The running host's version, or `None` when nothing healthy answers.
    /// A host that says only `ok` reports as `0.0.0`, which matches nothing.
    pub async fn version(&self) -> Option<String> {
        let text = self
            .http
            .get(format!("{}/healthz", self.base))
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .text()
            .await
            .ok()?;
        let mut words = text.split_whitespace();
        (words.next() == Some("ok")).then(|| words.next().unwrap_or("0.0.0").to_string())
    }

    /// Asks the host to stop and waits for its port to go quiet.
    ///
    /// Only a request, carrying the bearer token. There is no signal fallback:
    /// a pid from a file may belong to another process by now.
    pub async fn stop_and_wait(&self) {
        let _ = self
            .http
            .post(format!("{}/api/quit", self.base))
            .bearer_auth(&self.token)
            .timeout(std::time::Duration::from_millis(1000))
            .send()
            .await;
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !self.healthy().await {
                return;
            }
        }
    }

    pub async fn healthy(&self) -> bool {
        self.http
            .get(format!("{}/healthz", self.base))
            .timeout(std::time::Duration::from_millis(500))
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false)
    }

    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let res = self
            .http
            .get(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("GET {path}"))?;
        if !res.status().is_success() {
            bail!("{}", explain_status(path, res.status()));
        }
        res.json().await.with_context(|| format!("decoding {path}"))
    }

    pub async fn post<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let res = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        if !res.status().is_success() {
            bail!("{}", explain_status(path, res.status()));
        }
        res.json().await.with_context(|| format!("decoding {path}"))
    }

    /// POSTs a body and returns the decoded answer, including error bodies,
    /// because the host explains refusals in JSON.
    pub async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        self.post_json_inner(path, body, None).await
    }

    async fn post_json_inner<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
        within: Option<std::time::Duration>,
    ) -> Result<T> {
        let mut req = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(body);
        if let Some(d) = within {
            req = req.timeout(d);
        }
        let res = req.send().await.with_context(|| format!("POST {path}"))?;
        let status = res.status();
        let text = res
            .text()
            .await
            .with_context(|| format!("reading the answer to {path}"))?;
        match serde_json::from_str::<T>(&text) {
            Ok(v) => Ok(v),
            // A failed request whose body is not JSON carries a plain-text
            // reason; show it rather than a serde error and an internal route.
            Err(_) if !status.is_success() => {
                let said = text.trim();
                match said.is_empty() {
                    true => bail!("{}", explain_status(path, status)),
                    false => bail!("{said}"),
                }
            }
            Err(e) => Err(e).with_context(|| format!("decoding {path}")),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}
