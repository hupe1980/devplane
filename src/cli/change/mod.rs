//! `devplane change`: one function per verb, each in the file for what a
//! person is doing — starting, reading, steering, or deciding.

mod decide;
mod draft;
mod read;
mod review;
mod start;
mod steer;

use super::ChangeCmd;
use super::ids;
use anyhow::Result;

/// Refuses a decision about a change from inside an agent's session: an agent
/// may not offer, finish, archive or mark read its own work. The same check
/// `inbox answer` makes, with the same limit: it is advisory — an agent that
/// unsets the variable, or calls the host with the token in
/// `~/.devplane/token`, is not stopped by it.
fn refuse_from_agent(what: &str) -> Result<()> {
    if let Some(var) = crate::hook::agent_session() {
        anyhow::bail!(
            "refused: `{var}` is set, so this is running inside an agent's session, and an \
             agent may not {what} its own change. Do it from the Devplane window or a terminal \
             of your own. This check is advisory: an agent that unsets the variable, or calls \
             the host with its token, is not stopped by it."
        );
    }
    Ok(())
}

/// The exit status of a refusal, distinct from an error's 1.
const REFUSED: i32 = 3;

/// A host's answer to one request: what this command reads of it, as `T`,
/// beside the document as the host wrote it, which `--json` prints unchanged.
/// The document is carried, never looked into.
pub(super) struct Answer<T> {
    doc: serde_json::Value,
    /// The refusal the host explained in words, if it did.
    error: Option<String>,
    /// `T`, or why the document is not one — only ever kept beside `error`.
    read: std::result::Result<T, String>,
}

impl<T> Answer<T> {
    /// The refusal the host explained, if it did.
    fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// What the host answered, read whether or not it also refused.
    fn typed(&self) -> Option<&T> {
        self.read.as_ref().ok()
    }

    /// What the host answered, or its refusal as this command's error.
    fn read(&self) -> Result<&T> {
        if let Some(e) = &self.error {
            anyhow::bail!("{e}");
        }
        self.read.as_ref().map_err(|e| anyhow::anyhow!("{e}"))
    }
}

impl<'de, T: serde::de::DeserializeOwned> serde::Deserialize<'de> for Answer<T> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        /// The one field every refusal carries.
        #[derive(serde::Deserialize)]
        struct Said {
            #[serde(default)]
            error: Option<String>,
        }
        let doc = serde_json::Value::deserialize(d)?;
        let error = Said::deserialize(&doc).ok().and_then(|s| s.error);
        let read = T::deserialize(&doc).map_err(|e| e.to_string());
        // A refusal need not be a `T`; an answer that is neither is undecodable.
        if let (None, Err(e)) = (&error, &read) {
            return Err(serde::de::Error::custom(e));
        }
        Ok(Self { doc, error, read })
    }
}

impl<T> serde::Serialize for Answer<T> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        self.doc.serialize(s)
    }
}

/// An answer whose one sentence is all a command says of it.
#[derive(serde::Deserialize)]
pub(super) struct Says {
    #[serde(default)]
    says: Option<String>,
}

pub async fn cmd_change(what: ChangeCmd, json: bool) -> Result<()> {
    // Decisions a person takes, refused from an agent's session before
    // anything is read.
    match &what {
        ChangeCmd::Offer { .. } => refuse_from_agent("offer")?,
        ChangeCmd::Draft { .. } => refuse_from_agent("start the draft of")?,
        ChangeCmd::DraftAccept { .. } => refuse_from_agent("accept the draft of")?,
        ChangeCmd::DraftReject { .. } => refuse_from_agent("reject the draft of")?,
        ChangeCmd::Clarify { .. } => refuse_from_agent("answer the questions in")?,
        ChangeCmd::Resume { agent: Some(_), .. } => refuse_from_agent("switch the agent of")?,
        ChangeCmd::Login { .. } => refuse_from_agent("sign in the agent of")?,
        ChangeCmd::Finish { .. } => refuse_from_agent("finish")?,
        ChangeCmd::Archive { .. } => refuse_from_agent("archive")?,
        ChangeCmd::ClaimRead { .. } => refuse_from_agent("mark read the claim on")?,
        ChangeCmd::Review { seen, .. } if !seen.is_empty() => {
            refuse_from_agent("mark hunks seen in")?
        }
        _ => {}
    }
    match what {
        ChangeCmd::Start {
            title,
            agent,
            project,
            no_worktree,
            issue,
            spec,
            task,
        } => {
            start::cmd_start(
                start::Start {
                    title,
                    agent,
                    project,
                    no_worktree,
                    issue,
                    spec,
                    task,
                },
                json,
            )
            .await
        }
        ChangeCmd::Adopt {
            branch,
            project,
            spec,
            title,
        } => start::cmd_adopt(branch, project, spec, title, json).await,
        ChangeCmd::Draft {
            name,
            agent,
            methodology,
            project,
            change,
            dry_run,
            intent,
        } => {
            draft::cmd_draft(
                draft::Ask {
                    name,
                    agent,
                    methodology,
                    project,
                    change,
                    dry_run,
                    intent,
                },
                json,
            )
            .await
        }
        ChangeCmd::DraftAccept { change } => draft::cmd_decide(&change, true, json).await,
        ChangeCmd::DraftReject { change } => draft::cmd_decide(&change, false, json).await,
        ChangeCmd::Clarify {
            change,
            at,
            yes,
            answer,
        } => draft::cmd_clarify(&change, &at, yes, answer, json).await,
        ChangeCmd::List => read::cmd_list(json).await,
        ChangeCmd::Show { change } => read::cmd_show(&change, json).await,
        ChangeCmd::Export {
            change,
            out,
            force,
            sign,
        } => read::cmd_export(&change, json, out.as_deref(), force, sign.is_some()).await,
        ChangeCmd::Review { change, seen, .. } if !seen.is_empty() => {
            review::cmd_mark_seen(&change, &seen, json).await
        }
        ChangeCmd::Review { change, by, .. } => review::cmd_review(&change, &by, json).await,
        ChangeCmd::Verify {
            change,
            base_checks,
        } => steer::cmd_verify(&change, base_checks, json).await,
        ChangeCmd::Retry { change } => steer::cmd_retry(&change, json).await,
        ChangeCmd::Resume {
            change,
            agent,
            prompt,
        } => steer::cmd_resume(&change, agent.as_deref(), &prompt.join(" "), json).await,
        ChangeCmd::Login { change, method } => {
            steer::cmd_login(&change, method.as_deref(), json).await
        }
        ChangeCmd::Prompt { run, text } => steer::cmd_prompt(&run, text, json).await,
        ChangeCmd::Stop { run, yes } => steer::cmd_stop(&run, yes, json).await,
        ChangeCmd::Drift {
            change,
            run,
            accept,
            tell,
        } => steer::cmd_drift(&change, &run, accept, tell, json).await,
        ChangeCmd::Offer { change } => decide::cmd_offer(&change, json).await,
        ChangeCmd::ClaimRead { change } => decide::cmd_claim_read(&change, json).await,
        ChangeCmd::Finish { change } => decide::cmd_finish(&change, json).await,
        ChangeCmd::Archive {
            change,
            delete_branch,
            discard_uncommitted,
            force,
        } => decide::cmd_archive(&change, delete_branch, discard_uncommitted, force, json).await,
    }
}

/// A running host and the whole id of the change `needle` names: what every
/// verb that acts on a change starts with.
async fn host_and_change(needle: &str) -> Result<(crate::client::Client, String)> {
    let c = crate::client::Client::connect_running().await?;
    let id = ids::change(&crate::local::Reader::Host(c.clone()), needle).await?;
    Ok((c, id))
}
