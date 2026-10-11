//! Whether a newer Devplane exists, as the host serves it to the window.
//!
//! No I/O here and no network: the check itself lives in the app
//! (`src/app/update.rs`, compiled only with the window), which attaches a
//! [`Driver`] when it starts. A host without the window has none, and every
//! route answers *not this build*, so the command line can never check.
//!
//! The person's answer to *may Devplane check* is `updates` in their own
//! `app.toml`; until they answer, no request is made.

use crate::config::UpdatePref;
use crate::host::Shared;
use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::{Mutex, OnceLock};

/// The one address an update check fetches.
pub const ENDPOINT: &str =
    "https://github.com/hupe1980/devplane/releases/latest/download/latest.json";
/// Where releases are published, for every state that cannot install.
pub const RELEASES: &str = "https://github.com/hupe1980/devplane/releases";
/// What a check sends, said where the person is asked.
pub const SENDS: &str = "nothing about you, this machine or your projects";

/// Where the window's update check stands. Nothing is downloaded before
/// `Installing`, which only the person's *Install and restart* reaches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export, export_to = "wire/"))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum UpdateState {
    /// The host runs without the window (`devplane up --no-browser`).
    NotThisBuild,
    /// The person has not answered: no request is made.
    NotAsked { fetches: String, sends: String },
    /// Checks are off: no request is made.
    Off { releases: String },
    /// This build carries no release key, so nothing it fetched could be
    /// verified; it never checks.
    Unavailable { why: String, releases: String },
    /// The one check of this launch is in flight.
    Checking,
    /// The check found nothing newer.
    Current { running: String },
    /// The check did not complete (offline, the address did not answer).
    CheckFailed { why: String },
    /// The check found something it could not trust.
    Untrusted { why: String },
    /// A newer release; nothing downloaded.
    Offer {
        version: String,
        /// Plain text from the manifest; never rendered as markup.
        notes: String,
        running: String,
    },
    /// A newer release exists, but this install cannot replace itself.
    CannotReplace {
        version: String,
        why: String,
        #[serde(rename = "where")]
        #[cfg_attr(feature = "typescript", ts(rename = "where"))]
        where_to_get: String,
    },
    /// Downloading and verifying, on the person's word.
    Installing { version: String },
    /// Downloading or verifying failed; nothing was replaced.
    Failed { version: String, why: String },
    /// The person dismissed this version; it is not offered again unasked.
    Dismissed { version: String },
}

impl UpdateState {
    /// The state a preference starts in, before any check.
    pub fn for_pref(pref: Option<UpdatePref>) -> UpdateState {
        match pref {
            None => UpdateState::NotAsked {
                fetches: ENDPOINT.into(),
                sends: SENDS.into(),
            },
            Some(UpdatePref::Off) => UpdateState::Off {
                releases: RELEASES.into(),
            },
            Some(UpdatePref::On) => UpdateState::Checking,
        }
    }
}

/// What the app does on the person's word. Implemented only in the app.
pub trait Driver: Send + Sync {
    /// The preference changed: check if it is now on and this launch has not
    /// checked yet; discard any check in flight if it is now off.
    fn preference_changed(&self, pref: UpdatePref);
    /// Download, verify against the built-in key, then restart into it.
    fn install(&self, version: &str);
}

struct Updates {
    state: Mutex<UpdateState>,
    driver: Box<dyn Driver>,
}

static UPDATES: OnceLock<Updates> = OnceLock::new();

/// Called once by the app as it starts. A second call is ignored.
pub fn attach(initial: UpdateState, driver: Box<dyn Driver>) {
    let _ = UPDATES.set(Updates {
        state: Mutex::new(initial),
        driver,
    });
}

/// The state now: *not this build* where no app attached a driver.
pub fn current() -> UpdateState {
    UPDATES
        .get()
        .map(|u| {
            u.state
                .lock()
                .map(|s| s.clone())
                .unwrap_or(UpdateState::Checking)
        })
        .unwrap_or(UpdateState::NotThisBuild)
}

/// Sets the state; a no-op without the app.
pub fn set(next: UpdateState) {
    if let Some(u) = UPDATES.get()
        && let Ok(mut s) = u.state.lock()
    {
        *s = next;
    }
}

fn driver() -> Option<&'static dyn Driver> {
    UPDATES.get().map(|u| u.driver.as_ref())
}

fn refuse(status: StatusCode, body: serde_json::Value) -> Response {
    (status, Json(body)).into_response()
}

fn unauthorised() -> Response {
    refuse(StatusCode::UNAUTHORIZED, json!({"error": "unauthorised"}))
}

fn not_this_build() -> Response {
    refuse(
        StatusCode::NOT_FOUND,
        json!({
            "refused": "not_this_build",
            "says": format!("update checks are in the desktop app; releases are at {RELEASES}"),
        }),
    )
}

/// `GET /api/app/update`: the state now.
pub async fn state(State(st): State<Shared>, headers: HeaderMap) -> Response {
    if !crate::api::authorised(&st, &headers) {
        return unauthorised();
    }
    Json(current()).into_response()
}

#[derive(Deserialize)]
pub struct PrefBody {
    updates: UpdatePref,
    #[serde(default)]
    confirm: bool,
    /// The diff the person was shown; a file changed since is shown again.
    #[serde(default)]
    diff: Option<String>,
}

/// `POST /api/app/updates`: previews the `app.toml` change, and with
/// `confirm` writes exactly it.
pub async fn preference(
    State(st): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<PrefBody>,
) -> Response {
    if !crate::api::authorised(&st, &headers) {
        return unauthorised();
    }
    let Some(driver) = driver() else {
        return not_this_build();
    };
    let path = match crate::config::app_config_path() {
        Ok(p) => p,
        Err(e) => {
            return refuse(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({"error": format!("{e:#}")}),
            );
        }
    };
    let shown = match crate::config::preview_update_pref(&path, body.updates) {
        Ok(c) => c,
        Err(e) => {
            return refuse(
                StatusCode::CONFLICT,
                json!({"refused": "unreadable", "error": format!("{e:#}")}),
            );
        }
    };
    if !body.confirm {
        return Json(&shown).into_response();
    }
    if body.diff.as_deref().is_some_and(|d| d != shown.diff) {
        // The change as it is now, in the preview's own shape, so the window
        // shows it in place of the stale one and asks again.
        return refuse(
            StatusCode::CONFLICT,
            json!({
                "refused": "changed",
                "says": "app.toml changed since this was shown; this is the change now",
                "path": shown.path,
                "diff": shown.diff,
                "writes": shown.writes,
            }),
        );
    }
    match crate::config::write_update_pref(&path, body.updates, &shown) {
        Ok(true) => {
            driver.preference_changed(body.updates);
            Json(json!({"updates": body.updates})).into_response()
        }
        Ok(false) => refuse(
            StatusCode::CONFLICT,
            json!({"refused": "changed", "says": "app.toml changed while it was written; nothing was written"}),
        ),
        Err(e) => refuse(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({"error": format!("{e:#}")}),
        ),
    }
}

#[derive(Deserialize)]
pub struct VersionBody {
    version: String,
    #[serde(default)]
    confirm: bool,
}

/// *Install and restart*: refused where nothing is on offer or the install
/// cannot replace itself, and held for confirmation when the restart would
/// stop an agent, exactly as quitting is.
pub async fn install(
    State(st): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<VersionBody>,
) -> Response {
    if !crate::api::authorised(&st, &headers) {
        return unauthorised();
    }
    let Some(driver) = driver() else {
        return not_this_build();
    };
    match current() {
        UpdateState::Offer { version, .. }
        | UpdateState::Dismissed { version }
        | UpdateState::Failed { version, .. }
            if version == body.version => {}
        UpdateState::CannotReplace {
            why, where_to_get, ..
        } => {
            return refuse(
                StatusCode::CONFLICT,
                json!({"refused": "cannot_replace", "why": why, "where": where_to_get}),
            );
        }
        other => {
            return refuse(
                StatusCode::CONFLICT,
                json!({"refused": "no_offer", "says": format!("version {} is not on offer", body.version), "state": other}),
            );
        }
    }
    let snap = st.snapshot().await;
    let q =
        crate::core::reduce::facts::quitting(snap.world.runs(), &snap.open_asks, st.hold.summary());
    if (!q.stops.is_empty() || q.held.is_some()) && !body.confirm {
        return refuse(
            StatusCode::CONFLICT,
            json!({"refused": "stops", "stops": q.stops, "says": q.says()}),
        );
    }
    driver.install(&body.version);
    (
        StatusCode::ACCEPTED,
        Json(json!({"installing": body.version})),
    )
        .into_response()
}

/// *Not now*: the version is not offered again until the person opens the
/// update entry, where it still is.
pub async fn dismiss(
    State(st): State<Shared>,
    headers: HeaderMap,
    Json(body): Json<VersionBody>,
) -> Response {
    if !crate::api::authorised(&st, &headers) {
        return unauthorised();
    }
    if driver().is_none() {
        return not_this_build();
    }
    if matches!(current(), UpdateState::Offer { ref version, .. } if *version == body.version) {
        set(UpdateState::Dismissed {
            version: body.version,
        });
    }
    Json(current()).into_response()
}
