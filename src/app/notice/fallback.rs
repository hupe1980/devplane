//! A title and a body through the system's own notifier — the command line
//! host's path — where the app cannot use the platform's actionable
//! notifications: an unbundled macOS build, a Windows build with no
//! registered app id, a desktop with no notification service. No action, no
//! click, no withdrawal: the tray count and the window are the way in.

use super::{NoticeSink, Respond};
use crate::core::notice::{Capability, Notice};

pub struct PlainSink;

impl NoticeSink for PlainSink {
    fn capability(&self) -> Capability {
        Capability::PLAIN
    }

    fn deliver(&self, notice: &Notice, _respond: Respond) -> Result<(), String> {
        crate::notify::send(&format!("Devplane · {}", notice.title), &notice.body);
        Ok(())
    }

    fn withdraw(&self, _key: &str) {}

    fn describe(&self) -> String {
        "plain, no actions — this build cannot use the platform's actionable notifications; \
         the tray count and the window are the way in"
            .into()
    }
}
