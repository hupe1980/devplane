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
            .context("no daemon is running — start one with `vibeplane serve`")?;
        Ok(Self {
            base: info.base_url(),
            token: crate::config::load_or_create_token()?,
            http: reqwest::Client::new(),
        })
    }

    /// Connects, starting a daemon first if none is running.
    ///
    /// Every client command does this, so the daemon is something the user
    /// never has to think about: the first `vibeplane ls` after a reboot starts
    /// the observer that should have been running all along.
    pub async fn connect_or_start() -> Result<Self> {
        if let Ok(c) = Self::connect()
            && c.healthy().await
        {
            return Ok(c);
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
        bail!("started a daemon but it did not become ready; try `vibeplane serve` to see why")
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
