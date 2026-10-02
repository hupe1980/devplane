//! `devplane verify` — the arguments, and the hand-off to [`crate::verify`].

use anyhow::Result;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Clone, Debug)]
pub struct VerifyArgs {
    /// What the change is compared against: any commit-ish, tried as
    /// `origin/<ref>` then `<ref>`. Default: the branch `origin/HEAD` names,
    /// else `main`
    #[arg(long, value_name = "REF")]
    pub base: Option<String>,
    /// Write the Markdown summary here (created or replaced) instead of
    /// appending it to `$GITHUB_STEP_SUMMARY`
    #[arg(long, value_name = "PATH")]
    pub summary: Option<PathBuf>,
    /// Write the in-toto statement here when `check` ran
    #[arg(long, value_name = "PATH")]
    pub statement: Option<PathBuf>,
    /// Run `check` alone: skip the base's tests on this change and this
    /// change's new tests on the base, which run by default after a pass
    #[arg(long)]
    pub no_base_checks: bool,
}

pub async fn cmd_verify(a: VerifyArgs, json: bool) -> Result<()> {
    let args = crate::verify::Args {
        base: a.base,
        json,
        summary: a.summary,
        statement: a.statement,
        no_base_checks: a.no_base_checks,
    };
    let here = std::env::current_dir()?;
    match crate::verify::run(&here, &args).await {
        0 => Ok(()),
        code => Err(super::Exit(code).into()),
    }
}
