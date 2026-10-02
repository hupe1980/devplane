//! The one resolver every id argument goes through: a whole id, or any
//! unique prefix of one, for a change, a run or an ask.
//!
//! Resolved here, before any route is asked, so every command accepts the
//! same prefixes and refuses an ambiguous one with the same sentence.

use crate::local::Reader;
use anyhow::{Result, bail};

/// The one id `needle` names among `ids`: itself when it is one, else the
/// only id it prefixes. `Ok(None)` when it names none, so a caller can let
/// the host answer for something this list does not carry.
pub(crate) fn unique<'a>(
    noun: &str,
    needle: &str,
    ids: impl IntoIterator<Item = &'a str>,
) -> Result<Option<String>> {
    if needle.is_empty() {
        bail!("say which {noun}: an id, or a unique prefix of one");
    }
    let ids: Vec<&str> = ids.into_iter().collect();
    if ids.contains(&needle) {
        return Ok(Some(needle.to_string()));
    }
    let mut hits: Vec<&str> = ids
        .iter()
        .copied()
        .filter(|id| id.starts_with(needle))
        .collect();
    hits.sort_unstable();
    hits.dedup();
    match hits.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some((*one).to_string())),
        many => bail!(
            "`{needle}` matches {} {noun}s: {}. Use more of the id.",
            many.len(),
            many.join(", ")
        ),
    }
}

/// A change, by id or unique prefix. Every change is on the list, so naming
/// none is an error here.
pub(crate) async fn change(reader: &Reader, needle: &str) -> Result<String> {
    let all = reader.changes().await?;
    match unique("change", needle, all.iter().map(|v| v.change.id.as_str()))? {
        Some(id) => Ok(id),
        None => bail!("no change `{needle}` — `devplane change list` lists them, ids included"),
    }
}

/// A run, by id, unique prefix or name. A run the board does not carry (one
/// long ended) is passed through for the host to find or refuse.
pub(crate) async fn run(reader: &Reader, needle: &str) -> Result<String> {
    let board = reader.board(true).await?;
    // A session's name, or the suffix after its project, names it too.
    if let Some(r) = board.runs.iter().find(|r| {
        r.name.as_deref().is_some_and(|n| {
            n == needle || n.rsplit_once('-').is_some_and(|(_, tail)| tail == needle)
        })
    }) {
        return Ok(r.id.clone());
    }
    Ok(
        unique("run", needle, board.runs.iter().map(|r| r.id.as_str()))?
            .unwrap_or_else(|| needle.to_string()),
    )
}

/// A change or a run: a change id starts `c-`. A change is addressed as
/// itself; its runs are the host's to pick.
pub(crate) async fn change_or_run(reader: &Reader, needle: &str) -> Result<Subject> {
    match needle.starts_with("c-") {
        true => Ok(Subject::Change(change(reader, needle).await?)),
        false => Ok(Subject::Run(run(reader, needle).await?)),
    }
}

/// What a change-or-run argument named.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Subject {
    Change(String),
    Run(String),
}

impl Subject {
    pub(crate) fn id(&self) -> &str {
        match self {
            Subject::Change(id) | Subject::Run(id) => id,
        }
    }
}

/// An open ask, by id or unique prefix. One no longer open is passed through,
/// so the answer's refusal says what became of it.
pub(crate) async fn ask(reader: &Reader, needle: &str) -> Result<String> {
    let asks: Asks = super::raw_as(reader, "/api/asks").await?;
    let open = asks.open.iter().filter_map(|a| a.id.as_deref());
    Ok(unique("ask", needle, open)?.unwrap_or_else(|| needle.to_string()))
}

/// `/api/asks`, read only as far as the open asks' ids.
#[derive(serde::Deserialize)]
struct Asks {
    #[serde(default)]
    open: Vec<AskId>,
}

/// One open ask, by its id alone.
#[derive(serde::Deserialize)]
struct AskId {
    #[serde(default)]
    id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::unique;

    #[test]
    fn a_whole_id_wins_over_a_longer_one_it_prefixes() {
        let ids = ["c-01a0", "c-01a0ff"];
        assert_eq!(
            unique("change", "c-01a0", ids).unwrap(),
            Some("c-01a0".into())
        );
    }

    #[test]
    fn a_unique_prefix_names_its_one_id() {
        let ids = ["c-01a0dc12", "c-02bb"];
        assert_eq!(
            unique("change", "c-01a", ids).unwrap(),
            Some("c-01a0dc12".into())
        );
    }

    #[test]
    fn an_ambiguous_prefix_names_every_candidate() {
        let e = unique("change", "c-0", ["c-01", "c-02"])
            .unwrap_err()
            .to_string();
        assert!(
            e.contains("matches 2 changes") && e.contains("c-01") && e.contains("c-02"),
            "{e}"
        );
    }

    #[test]
    fn naming_nothing_is_none_and_naming_emptiness_is_refused() {
        assert_eq!(unique("run", "zz", ["r-1"]).unwrap(), None);
        assert!(unique("run", "", ["r-1"]).is_err());
    }
}
