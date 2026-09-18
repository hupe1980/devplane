//! The CLI's view of the daemon.
//!
//! Every command in the terminal is a client of the same API the browser uses.
//! That is deliberate: one surface to keep working, and anything a person can
//! see they can also script with `--json`.

use anyhow::{Context, Result, bail};
use serde::de::DeserializeOwned;

pub struct Client {
    base: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    /// Connects to a running daemon.
    pub fn connect() -> Result<Self> {
        let info = crate::config::read_daemon_info()?
            .context("no daemon is running — start one with `devplane serve`")?;
        Ok(Self {
            base: info.base_url(),
            token: crate::config::load_or_create_token()?,
            http: reqwest::Client::new(),
        })
    }

    /// Connects, starting a daemon first if none is running.
    ///
    /// Every client command does this, so the daemon is something the user
    /// never has to think about: the first `devplane ls` after a reboot starts
    /// the observer that should have been running all along.
    pub async fn connect_or_start() -> Result<Self> {
        if let Ok(c) = Self::connect() {
            match c.version().await {
                Some(v) if v == env!("CARGO_PKG_VERSION") => return Ok(c),
                // A daemon from a previous install. Every command would then
                // hit routes it does not have and report 404 as if the feature
                // were missing — which is exactly what happened for two
                // releases. Restart it; the store and the spool survive.
                Some(v) => {
                    eprintln!(
                        "devplane: the running daemon is v{v} and this is v{}; restarting it",
                        env!("CARGO_PKG_VERSION")
                    );
                    c.stop_and_wait().await;
                }
                None => {}
            }
        }
        crate::daemonise::spawn_detached()?;
        for _ in 0..50 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if let Ok(c) = Self::connect()
                && c.healthy().await
            {
                return Ok(c);
            }
        }
        bail!("started a daemon but it did not become ready; try `devplane serve` to see why")
    }

    /// The running daemon's version, or `None` when nothing healthy answers.
    ///
    /// `/healthz` says `ok <version>`; a daemon old enough to say only `ok`
    /// reports as `0.0.0`, which is older than anything and gets restarted.
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

    /// Asks the daemon to stop and waits for its port to go quiet. Falls back
    /// to a signal for a daemon too old to have the route.
    async fn stop_and_wait(&self) {
        let _ = self
            .http
            .post(format!("{}/api/shutdown", self.base))
            .bearer_auth(&self.token)
            .timeout(std::time::Duration::from_millis(1000))
            .send()
            .await;
        for _ in 0..30 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !self.healthy().await {
                crate::config::clear_daemon_info().ok();
                return;
            }
        }
        #[cfg(unix)]
        if let Ok(Some(info)) = crate::config::read_daemon_info() {
            unsafe {
                libc::kill(info.pid as i32, libc::SIGTERM);
            }
            for _ in 0..30 {
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                if !self.healthy().await {
                    break;
                }
            }
        }
        crate::config::clear_daemon_info().ok();
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
            bail!("{path} returned {}", res.status());
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
            bail!("{path} returned {}", res.status());
        }
        res.json().await.with_context(|| format!("decoding {path}"))
    }

    /// POSTs a body and returns the decoded answer, including error bodies:
    /// the daemon explains refusals in JSON, and swallowing that to raise a
    /// status code would lose the explanation.
    pub async fn post_json<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &serde_json::Value,
    ) -> Result<T> {
        let res = self
            .http
            .post(format!("{}{path}", self.base))
            .bearer_auth(&self.token)
            .json(body)
            .send()
            .await
            .with_context(|| format!("POST {path}"))?;
        res.json().await.with_context(|| format!("decoding {path}"))
    }

    pub fn base_url(&self) -> &str {
        &self.base
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}
