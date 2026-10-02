//! The update check: the one request the app's own process makes beyond
//! loopback, and only after the person switched it on.
//!
//! Once per launch at most, after the window opens; never on a timer. A newer
//! release is held as an offer with nothing downloaded. *Install and restart*
//! downloads it, verifies it against the release key this build carries, and
//! only then stops the host the way quitting does and installs. Anything that
//! does not verify is discarded and said.

use crate::config::UpdatePref;
use crate::update::{self, RELEASES, UpdateState};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use tauri::{AppHandle, Manager};
use tauri_plugin_updater::UpdaterExt;

/// A verified download waiting for the host to stop: installed, then the app
/// restarts into it, by [`finish`].
static PENDING: Mutex<Option<(tauri_plugin_updater::Update, Vec<u8>)>> = Mutex::new(None);

/// Whether the host is stopping to install an update rather than to quit.
static RESTARTING: AtomicBool = AtomicBool::new(false);

struct AppDriver {
    app: AppHandle,
    /// A check is in flight.
    in_flight: AtomicBool,
    /// What this launch's check found, kept so turning checks off and on
    /// again shows it rather than asking a second time.
    last: Mutex<Option<UpdateState>>,
    /// Bumped when the person turns checks off, so a check in flight is
    /// discarded.
    generation: AtomicU64,
    /// The update the last check found, so installing fetches no manifest.
    found: Mutex<Option<tauri_plugin_updater::Update>>,
}

/// Starts the update state for this launch. With no release key built in
/// nothing could be verified, so it never checks.
pub fn start(app: &AppHandle, pref: Option<UpdatePref>) {
    let initial = if release_key(app).is_none() {
        UpdateState::Unavailable {
            why: "this build carries no release key, so it could verify nothing it fetched; \
                  it never checks for updates"
                .into(),
            releases: RELEASES.into(),
        }
    } else {
        UpdateState::for_pref(pref)
    };
    let check_now = matches!(initial, UpdateState::Checking);
    let driver = std::sync::Arc::new(AppDriver {
        app: app.clone(),
        in_flight: AtomicBool::new(false),
        last: Mutex::new(None),
        generation: AtomicU64::new(0),
        found: Mutex::new(None),
    });
    update::attach(initial, Box::new(Shared(driver.clone())));
    if check_now {
        driver.check();
    }
}

/// The public half of the release key this build carries, if any.
fn release_key(app: &AppHandle) -> Option<String> {
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_string)
}

/// Why this installation cannot replace itself, if it cannot: a `.deb` is the
/// package manager's, and an app run from a disk image or a translocated
/// copy has nowhere to write.
pub fn cannot_replace(
    os: &str,
    exe: &std::path::Path,
    appimage: Option<&std::ffi::OsStr>,
) -> Option<&'static str> {
    let p = exe.to_string_lossy();
    match os {
        "linux" if appimage.is_none_or(|a| a.is_empty()) => {
            Some("installed by the system's package manager, which updates it")
        }
        "macos" if p.contains("/AppTranslocation/") || p.starts_with("/Volumes/") => Some(
            "running from a disk image or a quarantined copy; move Devplane to Applications first",
        ),
        _ => None,
    }
}

struct Shared(std::sync::Arc<AppDriver>);

impl update::Driver for Shared {
    fn preference_changed(&self, pref: UpdatePref) {
        if release_key(&self.0.app).is_none() {
            return;
        }
        match pref {
            UpdatePref::Off => {
                self.0.generation.fetch_add(1, Ordering::SeqCst);
                update::set(UpdateState::for_pref(Some(UpdatePref::Off)));
            }
            UpdatePref::On => {
                let last = self.0.last.lock().expect("last").clone();
                match last {
                    // This launch already checked: its answer stands.
                    Some(found) => update::set(found),
                    None => self.0.check(),
                }
            }
        }
    }

    fn install(&self, version: &str) {
        let driver = self.0.clone();
        let version = version.to_string();
        tauri::async_runtime::spawn(async move { driver.install(version).await });
    }
}

impl AppDriver {
    fn check(self: &std::sync::Arc<Self>) {
        if self.in_flight.swap(true, Ordering::SeqCst) {
            return;
        }
        let me = self.clone();
        let generation = self.generation.load(Ordering::SeqCst);
        update::set(UpdateState::Checking);
        tauri::async_runtime::spawn(async move {
            let result = match me.app.updater() {
                Ok(u) => u.check().await,
                Err(e) => Err(e),
            };
            me.in_flight.store(false, Ordering::SeqCst);
            // Turned off while it ran: the result is discarded.
            if me.generation.load(Ordering::SeqCst) != generation {
                return;
            }
            let next = match result {
                Ok(None) => UpdateState::Current {
                    running: env!("CARGO_PKG_VERSION").into(),
                },
                Ok(Some(found)) => {
                    let exe = std::env::current_exe().unwrap_or_default();
                    let appimage = std::env::var_os("APPIMAGE");
                    match cannot_replace(std::env::consts::OS, &exe, appimage.as_deref()) {
                        Some(why) => UpdateState::CannotReplace {
                            version: found.version.clone(),
                            why: why.into(),
                            where_to_get: RELEASES.into(),
                        },
                        None => {
                            let offer = UpdateState::Offer {
                                version: found.version.clone(),
                                notes: found.body.clone().unwrap_or_default(),
                                running: found.current_version.clone(),
                            };
                            *me.found.lock().expect("found") = Some(found);
                            offer
                        }
                    }
                }
                Err(e) => describe(&e),
            };
            *me.last.lock().expect("last") = Some(next.clone());
            update::set(next);
        });
    }

    async fn install(&self, version: String) {
        let found = self.found.lock().expect("found").clone();
        let Some(found) = found.filter(|f| f.version == version) else {
            update::set(UpdateState::Failed {
                version,
                why: "the offer is gone; nothing was downloaded".into(),
            });
            return;
        };
        update::set(UpdateState::Installing {
            version: version.clone(),
        });
        // `download` verifies the signature against the built-in key before
        // it returns a byte; a failure leaves nothing to install.
        match found.download(|_, _| {}, || {}).await {
            Ok(bytes) => {
                *PENDING.lock().expect("pending") = Some((found, bytes));
                RESTARTING.store(true, Ordering::SeqCst);
                // Stop the host the way quitting does: driven agents are
                // stopped and `host.json` removed before anything is replaced.
                let hosted = self.app.state::<super::Hosted>();
                let _ = hosted.state.stopping.send(true);
            }
            Err(e) => {
                let why = match describe(&e) {
                    UpdateState::Untrusted { why } | UpdateState::CheckFailed { why } => why,
                    _ => e.to_string(),
                };
                update::set(UpdateState::Failed {
                    version,
                    why: format!("{why}; nothing was replaced"),
                });
            }
        }
    }
}

/// A failed check or download, said in words: a signature that does not
/// verify is *untrusted*; anything else did not complete.
fn describe(e: &tauri_plugin_updater::Error) -> UpdateState {
    use tauri_plugin_updater::Error as E;
    match e {
        E::Minisign(_) | E::Base64(_) | E::SignatureUtf8(_) => UpdateState::Untrusted {
            why: format!("the update's signature did not verify against this build's key ({e})"),
        },
        _ => UpdateState::CheckFailed {
            why: format!("the check did not complete: {e}"),
        },
    }
}

/// After the host stopped: if it stopped to update, install the verified
/// download and restart into it. Returns only when there was nothing to do.
pub fn finish(app: &AppHandle) {
    if !RESTARTING.load(Ordering::SeqCst) {
        return;
    }
    if let Some((update, bytes)) = PENDING.lock().expect("pending").take()
        && let Err(e) = update.install(bytes)
    {
        tracing::error!(error = %e, "installing the update failed; restarting the version that ran");
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::cannot_replace;
    use std::ffi::OsStr;
    use std::path::Path;

    /// The app never offers to replace what it cannot: a system package, a
    /// disk image, a translocated copy.
    #[test]
    fn what_cannot_replace_itself_says_so() {
        let deb = Path::new("/usr/bin/devplane");
        assert!(cannot_replace("linux", deb, None).is_some());
        let image = Path::new("/tmp/.mount_x/usr/bin/devplane");
        assert!(cannot_replace("linux", image, Some(OsStr::new("/home/u/D.AppImage"))).is_none());
        for p in [
            "/Volumes/Devplane/Devplane.app/Contents/MacOS/devplane",
            "/private/var/folders/x/AppTranslocation/A/d/Devplane.app/Contents/MacOS/devplane",
        ] {
            let why = cannot_replace("macos", Path::new(p), None).expect(p);
            assert!(why.contains("move Devplane to Applications"), "{why}");
        }
        let installed = Path::new("/Applications/Devplane.app/Contents/MacOS/devplane");
        assert!(cannot_replace("macos", installed, None).is_none());
        assert!(
            cannot_replace(
                "windows",
                Path::new("C:/Users/u/AppData/Local/Devplane/devplane.exe"),
                None
            )
            .is_none()
        );
    }
}
