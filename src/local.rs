//! Reading with nothing running.
//!
//! A command that only reads or records opens the SQLite file the hook and the
//! host write, assembles a [`Snapshot`], and composes the same surfaces the
//! host serves. What only a running host knows (driven sessions, the forge,
//! the gate's last probe) is marked absent rather than rendered as calm.
//! [`Reader`] prefers a host that answers, else the store; neither starts one.

use crate::client::Client;
use crate::store::Store;
use crate::view::{self, Snapshot};
use anyhow::{Context, Result};
use serde_json::{Value, json};

/// The store, the world restored from it, and the policy cache: everything a
/// read needs without a process.
pub struct Local {
    pub store: Store,
    pub policy: crate::core::PolicyCache,
    home: std::path::PathBuf,
}

impl Local {
    pub async fn open() -> Result<Self> {
        let home = crate::config::home()?;
        let store = Store::open(&crate::config::db_path()?)
            .await
            .context("opening the store")?;
        let (policy, _) = crate::core::PolicyCache::from_disk();
        Ok(Self {
            store,
            policy,
            home,
        })
    }

    /// The world as the store last saw it. A run recorded as working is shown
    /// as recorded; only a host can check the process, so there is no live set.
    pub async fn snapshot(&self) -> Result<Snapshot> {
        let mut world = crate::core::World::new();
        for p in self.store.load_projects().await? {
            world.upsert_project(p);
        }
        world.restore_runs(self.store.load_runs().await?);
        // Replay what hooks wrote past the host's mark, in memory only (the
        // mark is the host's to move). Paged to the end, and each event at
        // most once per run, so a run saved before the mark moved is not
        // counted twice.
        let mut through = self.store.projected_through().await?;
        loop {
            let pending = self.store.shim_events_since(through, 5_000).await?;
            let Some((last, _)) = pending.last() else {
                break;
            };
            through = *last;
            let n = pending.len();
            world.replay_stored(pending);
            if n < 5_000 {
                break;
            }
        }
        // With no host, `verified` is computed from the tree now rather than
        // trusting a stamp from some earlier moment.
        let mut changes = self.store.load_changes().await?;
        for c in changes.iter_mut().filter(|c| c.archived_at.is_none()) {
            // In place, the tree is the person's checkout.
            let dir = c
                .worktree
                .clone()
                .or_else(|| world.project(&c.project_id).map(|p| p.root.clone()));
            if let Some(dir) = dir {
                c.tree_now = crate::git::commit_stamp(&dir).await;
            }
        }
        let open_asks = self.store.open_asks().await?;
        Ok(Snapshot {
            world,
            changes,
            open_asks,
            live: Default::default(),
            forge: None,
            gate: None,
            broken_configs: self.policy.broken(),
            unwritten: (0, 0, None),
            leaked_agents: Vec::new(),
            agents: crate::acp::available(&self.home),
            from_host: false,
            now: jiff::Timestamp::now(),
            started_at: None,
        })
    }

    /// Answers a read the way the host would, addressed by the same path.
    pub async fn get(&self, path: &str) -> Result<Value> {
        let (route, query) = path.split_once('?').unwrap_or((path, ""));
        let q = |key: &str| -> Option<String> {
            query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(k, _)| *k == key)
                .map(|(_, v)| url_unescape(v))
        };
        let flag = |key: &str| q(key).is_some_and(|v| v == "true" || v == "1");
        let limit = q("limit").and_then(|l| l.parse().ok()).unwrap_or(200);
        let parts: Vec<&str> = route.trim_start_matches('/').split('/').collect();
        let snap = self.snapshot().await?;

        Ok(match parts.as_slice() {
            ["api", "board"] => {
                serde_json::to_value(view::board(&snap, flag("all"), &self.policy.broken()))?
            }
            ["api", "inbox"] => {
                let q = view::InboxQuery {
                    read: flag("read"),
                    project: q("project"),
                    needs_you: flag("needs_you"),
                };
                view::inbox(&snap, &self.store, &self.policy, &q).await
            }
            ["api", "asks"] => {
                let recent = self.store.asks(50).await.unwrap_or_default();
                view::asks(&snap, &recent)
            }
            ["api", "runs", id] => {
                let id = snap
                    .world
                    .resolve_run(id)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                view::run_detail(snap.world.run(&id).context("no such run")?)
            }
            ["api", "runs", id, "events"] => {
                let id = snap
                    .world
                    .resolve_run(id)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                serde_json::to_value(self.store.events_for_run(&id, limit).await?)?
            }
            ["api", "runs", id, "messages"] => {
                let id = snap
                    .world
                    .resolve_run(id)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                serde_json::to_value(self.store.messages_for_run(&id, limit).await?)?
            }
            ["api", "modes"] => serde_json::to_value(view::modes(&snap))?,
            ["api", "attention"] => {
                let days = q("days").and_then(|d| d.parse().ok()).unwrap_or(7);
                view::attention(&self.store, days).await?
            }
            ["api", "search"] => {
                view::search(&self.store, &q("q").unwrap_or_default(), limit).await?
            }
            ["api", "decisions"] => serde_json::to_value(
                view::decisions(
                    &self.store,
                    q("about").as_deref(),
                    limit,
                    flag("without_me"),
                )
                .await?,
            )?,
            ["api", "agents"] => json!(view::agents(&snap, &self.store).await),
            ["api", "changes"] => {
                serde_json::to_value(view::change_list(&snap, &self.store).await)?
            }
            ["api", "changes", id] => {
                let id = snap.resolve_change(id).map_err(|e| anyhow::anyhow!(e))?;
                view::change_one(&snap, &self.store, &id)
                    .await
                    .context("no such change")?
            }
            // Either resumes or re-baselines a run, which only a host can do.
            ["api", "changes", _, "drift", _] => {
                anyhow::bail!("`{route}` needs a running host — start one with `devplane serve`")
            }
            ["api", "changes", id, "review"] => {
                let id = snap.resolve_change(id).map_err(|e| anyhow::anyhow!(e))?;
                serde_json::to_value(
                    view::review(&snap, &self.store, &id)
                        .await
                        .map_err(|why| anyhow::anyhow!(why.says()))?,
                )?
            }
            ["api", "changes", id, "certificate"] => {
                let id = snap.resolve_change(id).map_err(|e| anyhow::anyhow!(e))?;
                view::change_certificate(&snap, &self.store, &id)
                    .await
                    .context("no such change")?
            }
            ["api", "reports"] => {
                let rq = view::ReportQuery {
                    to: q("to"),
                    from: q("from"),
                    all: flag("all"),
                };
                serde_json::to_value(view::reports(&snap, &self.store, &rq).await?)?
            }
            ["api", "reports", id] => serde_json::to_value(
                view::report(&snap, &self.store, id)
                    .await?
                    .with_context(|| format!("no report `{id}`"))?,
            )?,
            ["api", "specs"] => view::specs(&snap),
            ["api", "projects"] => json!(view::projects(&snap)),
            ["api", "setup"] => view::setup(&snap),
            ["api", "rules"] => {
                view::rules(&snap, q("rule").as_deref(), flag("ask")).unwrap_or_else(|e| e)
            }
            ["api", "quitting"] => serde_json::to_value(crate::core::reduce::facts::quitting(
                snap.world.runs(),
                &snap.open_asks,
            ))?,
            _ => anyhow::bail!("`{route}` needs a running host — start one with `devplane serve`"),
        })
    }
}

/// Where a read command gets its answers: a host that answers, else the
/// store. Neither is started by asking.
pub enum Reader {
    Host(Client),
    Store(Local),
}

impl Reader {
    pub async fn open() -> Result<Self> {
        if let Ok(c) = Client::connect()
            && c.healthy().await
        {
            return Ok(Reader::Host(c));
        }
        Ok(Reader::Store(Local::open().await?))
    }

    pub async fn get(&self, path: &str) -> Result<Value> {
        match self {
            Reader::Host(c) => c
                .get(path)
                .await
                .with_context(|| format!("fetching {path}")),
            Reader::Store(l) => l.get(path).await,
        }
    }

    pub async fn get_as<T: serde::de::DeserializeOwned>(&self, path: &str) -> Result<T> {
        let v = self.get(path).await?;
        serde_json::from_value(v).with_context(|| format!("reading {path}"))
    }

    /// Answers an ask: through the host where one runs, so a waiting agent
    /// hears it; otherwise straight into the store, where a held permission's
    /// hook is polling and anything else waits for a host to resume its run.
    pub async fn answer(&self, ask: &str, body: &Value) -> Result<Value> {
        match self {
            Reader::Host(c) => c.post_json(&format!("/api/asks/{ask}/answer"), body).await,
            Reader::Store(l) => {
                let row = l.store.ask(ask).await?.with_context(|| {
                    format!(
                        "no ask `{ask}` is waiting.\n\n  devplane asks lists every question and \
                         what became of it."
                    )
                })?;
                let s = |k: &str| body.get(k).and_then(|v| v.as_str()).map(str::to_string);
                let answers: Vec<crate::driven::FieldAnswer> = body
                    .get("answers")
                    .cloned()
                    .map(serde_json::from_value)
                    .transpose()?
                    .unwrap_or_default();
                let answer = crate::driven::parse_answer(
                    &row,
                    s("decision").as_deref(),
                    s("option"),
                    s("custom"),
                    s("field"),
                    &answers,
                )
                .map_err(|e| anyhow::anyhow!(e))?;
                let from = s("from").unwrap_or_else(|| "cli".into());
                let done = crate::driven::answer_without_host(&l.store, ask, answer, &from).await?;
                Ok(view::render_ask(&done))
            }
        }
    }

    /// Files a report: through the host where one runs, so a live target agent
    /// that named this source in `deliver_from` is handed it; else the store.
    pub async fn file_report(&self, body: &Value) -> Result<Value> {
        match self {
            Reader::Host(c) => c.post_json("/api/reports", body).await,
            Reader::Store(l) => {
                let req: crate::reports::FileRequest = serde_json::from_value(body.clone())?;
                let filed = crate::reports::file(&l.store, req)
                    .await
                    .map_err(|r| anyhow::anyhow!(r.says()))?;
                Ok(crate::api::filed_json(filed))
            }
        }
    }

    pub fn from_host(&self) -> bool {
        matches!(self, Reader::Host(_))
    }

    /// The sentence a command prints, dimly, when it read the store; shared
    /// with the board.
    pub fn limits(&self) -> Option<&'static str> {
        match self {
            Reader::Host(_) => None,
            Reader::Store(_) => Some(
                "read from the store — no host is running, so live sessions, GitHub and the \
                 gate probe are not visible from here; `devplane serve` starts one",
            ),
        }
    }
}

/// The inverse of `core::text::url_escape`, for the query a command wrote.
fn url_unescape(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex = &s[i + 1..i + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(b) => {
                        out.push(b);
                        i += 3;
                    }
                    _ => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::url_unescape;

    #[test]
    fn a_query_value_comes_back_as_it_was_typed() {
        assert_eq!(url_unescape("rm%20-rf%20%2F"), "rm -rf /");
        assert_eq!(url_unescape("caf%C3%A9"), "café");
        assert_eq!(url_unescape("a+b"), "a b");
        // A stray `%` is a character, not a crash.
        assert_eq!(url_unescape("100%"), "100%");
        assert_eq!(url_unescape("%zz"), "%zz");
    }
}
