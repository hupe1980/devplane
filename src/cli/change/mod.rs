//! `devplane change`: one function per verb, each in the file for what a
//! person is doing — starting, reading, steering, or deciding.

mod decide;
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

/// The error a host explained in its answer, if it did.
fn said_error(v: &serde_json::Value) -> Option<&str> {
    v.get("error").and_then(|e| e.as_str())
}

pub async fn cmd_change(what: ChangeCmd, json: bool) -> Result<()> {
    // Decisions a person takes, refused from an agent's session before
    // anything is read.
    match &what {
        ChangeCmd::Offer { .. } => refuse_from_agent("offer")?,
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
        ChangeCmd::Resume { change } => steer::cmd_resume(&change, json).await,
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
