//! The tray item: the number, and the two things a menu needs.
//!
//! A number, never a dot: the inbox's *needs you* count, the same figure
//! `devplane inbox --needs-you` prints. Zero shows nothing, not `0`.

use super::{quit, window};
use crate::core::AttentionItem;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{TrayIcon, TrayIconBuilder};
use tauri::{AppHandle, Manager};

/// How many things need the person right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count(pub usize);

impl Count {
    /// Beside the icon on macOS and Linux; nothing at zero.
    pub fn title(&self) -> Option<String> {
        (self.0 > 0).then(|| self.0.to_string())
    }

    /// The dock badge; the same rule.
    pub fn badge(&self) -> Option<i64> {
        (self.0 > 0).then_some(self.0 as i64)
    }

    /// On hover, and on Windows, where the tray shows no title.
    pub fn tooltip(&self) -> String {
        match self.0 {
            0 => "nothing needs you".to_string(),
            1 => "1 needs you".to_string(),
            n => format!("{n} need you"),
        }
    }
}

/// The count: the rows a person can answer from here.
///
/// The same predicate `--needs-you` narrows by: has an answer path.
pub fn needs_you(items: &[AttentionItem]) -> Count {
    Count(items.iter().filter(|i| i.has_answer_path()).count())
}

const OPEN: &str = "open";
const QUIT: &str = "quit";

/// The tray item with its menu: *Open* shows the window, *Quit* asks.
pub fn build(app: &AppHandle) -> tauri::Result<TrayIcon> {
    let icon = tauri::image::Image::from_bytes(include_bytes!("../../icons/32x32.png"))?;
    let open = MenuItem::with_id(app, OPEN, "Open Devplane", true, None::<&str>)?;
    let stop = MenuItem::with_id(app, QUIT, "Quit Devplane", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &PredefinedMenuItem::separator(app)?, &stop])?;
    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip(Count(0).tooltip())
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| match e.id().as_ref() {
            OPEN => window::show_main(app, ""),
            QUIT => quit::ask(app),
            _ => {}
        })
        .build(app)
}

/// Writes the count where the platform draws attention: the tray's title
/// and tooltip, and the dock badge where there is a dock.
pub fn apply(app: &AppHandle, count: Count) {
    let hosted = app.state::<super::Hosted>();
    if let Some(tray) = hosted.tray.lock().expect("tray").as_ref() {
        let _ = tray.set_title(count.title());
        let _ = tray.set_tooltip(Some(count.tooltip()));
    }
    // Unsupported on Windows, where the overlay icon is the equivalent and
    // the tooltip already carries the number.
    #[cfg(not(windows))]
    if let Some(w) = app.get_webview_window(window::MAIN) {
        let _ = w.set_badge_count(count.badge());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::attention::{Action, AttentionKind};
    use crate::core::ids::{AttentionId, RunId};

    fn item(id: &str, actions: Vec<Action>) -> AttentionItem {
        AttentionItem {
            id: AttentionId::new(id),
            kind: AttentionKind::Question,
            level: AttentionKind::Question.default_level(),
            run_id: Some(RunId::new("r")),
            project_id: None,
            title: "Keep the legacy route?".into(),
            detail: None,
            answer_in: None,
            options: vec![],
            actions,
            ask: None,
            request_id: None,
            form: None,
            url: None,
            launch: None,
            change_id: None,
            offer: None,
            no_offer: None,
            report: None,
            since: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn zero_shows_nothing_and_three_shows_three() {
        assert_eq!(Count(0).title(), None);
        assert_eq!(Count(0).badge(), None);
        assert_eq!(Count(0).tooltip(), "nothing needs you");
        assert_eq!(Count(3).title().as_deref(), Some("3"));
        assert_eq!(Count(3).badge(), Some(3));
        assert_eq!(Count(3).tooltip(), "3 need you");
        assert_eq!(Count(1).tooltip(), "1 needs you");
    }

    #[test]
    fn the_count_is_the_answerable_rows() {
        assert_eq!(needs_you(&[]), Count(0));
        let answerable = || item("a", vec![Action::Allow, Action::Deny]);
        // A row with nothing to press is not something to answer.
        let mute = item("m", vec![]);
        assert_eq!(
            needs_you(&[answerable(), answerable(), answerable(), mute]),
            Count(3)
        );
    }
}
