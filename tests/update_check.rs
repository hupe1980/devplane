//! The update routes the window's Setup uses: a host without the window says
//! *not this build*; with the window the preference starts *not asked*, is
//! previewed before it is written, and writes one line of the person's own
//! `app.toml`. Its own binary: it sets the process's home and the one update
//! driver a process may have.

mod common;

use common::{get, post};
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

/// What the app would do, recorded instead.
struct Recorder(Arc<Mutex<Vec<String>>>);

impl devplane::update::Driver for Recorder {
    fn preference_changed(&self, pref: devplane::config::UpdatePref) {
        self.0
            .lock()
            .unwrap()
            .push(format!("pref {}", pref.as_str()));
    }
    fn install(&self, version: &str) {
        self.0.lock().unwrap().push(format!("install {version}"));
    }
}

async fn status_of(c: &reqwest::Client, url: String, body: Value) -> (u16, Value) {
    let r = c
        .post(url)
        .bearer_auth("tok")
        .json(&body)
        .send()
        .await
        .unwrap();
    (r.status().as_u16(), r.json().await.unwrap_or(Value::Null))
}

#[tokio::test]
async fn the_update_preference_is_asked_previewed_and_then_written() {
    let home = std::env::temp_dir().join(format!(
        "dp-update-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    std::fs::create_dir_all(&home).unwrap();
    // SAFETY: this binary holds one test; nothing else reads the variable.
    unsafe { std::env::set_var("DEVPLANE_HOME", &home) };
    let (addr, c, _host) = common::boot().await;

    // Without the window, every update route says so; no check exists.
    assert_eq!(
        get(&c, &addr, "/api/app/update").await["state"],
        "not_this_build"
    );
    let (code, v) = status_of(
        &c,
        format!("http://{addr}/api/app/updates"),
        json!({"updates": "on", "confirm": true}),
    )
    .await;
    assert_eq!((code, v["refused"].as_str()), (404, Some("not_this_build")));
    assert!(!home.join("app.toml").exists());

    // The app attaches its driver; a fresh home has not been asked.
    let seen = Arc::new(Mutex::new(Vec::new()));
    devplane::update::attach(
        devplane::update::UpdateState::for_pref(None),
        Box::new(Recorder(seen.clone())),
    );
    let v = get(&c, &addr, "/api/app/update").await;
    assert_eq!(v["state"], "not_asked");
    assert_eq!(v["fetches"], devplane::update::ENDPOINT);
    assert!(v["sends"].as_str().unwrap().contains("nothing about you"));

    // With no file, the preview adds the section too; it writes nothing.
    let fresh = post(
        &c,
        &addr,
        "/api/app/updates",
        json!({"updates": "on", "confirm": false}),
    )
    .await;
    assert_eq!(fresh["diff"], "+[app]\n+updates = \"on\"\n");
    assert!(!home.join("app.toml").exists(), "a preview wrote the file");

    // A preview of the person's own file shows the one line.
    std::fs::write(home.join("app.toml"), "[app]\nport = 47000 # mine\n").unwrap();
    let shown = post(
        &c,
        &addr,
        "/api/app/updates",
        json!({"updates": "on", "confirm": false}),
    )
    .await;
    assert_eq!(shown["diff"], "+updates = \"on\"\n");
    assert_eq!(shown["writes"], true);
    assert!(seen.lock().unwrap().is_empty());

    // Confirmed with the diff shown: exactly that is written.
    let (code, v) = status_of(
        &c,
        format!("http://{addr}/api/app/updates"),
        json!({"updates": "on", "confirm": true, "diff": shown["diff"]}),
    )
    .await;
    assert_eq!(code, 200, "{v}");
    assert_eq!(
        std::fs::read_to_string(home.join("app.toml")).unwrap(),
        "[app]\nupdates = \"on\"\nport = 47000 # mine\n"
    );
    assert_eq!(*seen.lock().unwrap(), vec!["pref on".to_string()]);

    // A diff that no longer matches is shown again, not written.
    let (code, v) = status_of(
        &c,
        format!("http://{addr}/api/app/updates"),
        json!({"updates": "off", "confirm": true, "diff": "+updates = \"off\"\n"}),
    )
    .await;
    assert_eq!((code, v["refused"].as_str()), (409, Some("changed")), "{v}");
    assert_eq!(
        v["diff"], "-updates = \"on\"\n+updates = \"off\"\n",
        "the change now is shown"
    );
    assert!(
        std::fs::read_to_string(home.join("app.toml"))
            .unwrap()
            .contains("\"on\"")
    );

    // Nothing is on offer, so nothing installs.
    let (code, v) = status_of(
        &c,
        format!("http://{addr}/api/app/update/install"),
        json!({"version": "9.9.9"}),
    )
    .await;
    assert_eq!((code, v["refused"].as_str()), (409, Some("no_offer")));

    // An offer installs on the person's word: nothing runs, so nothing to confirm.
    devplane::update::set(devplane::update::UpdateState::Offer {
        version: "9.9.9".into(),
        notes: "**not markup**".into(),
        running: "0.12.0".into(),
    });
    let v = get(&c, &addr, "/api/app/update").await;
    assert_eq!(v["notes"], "**not markup**");
    let (code, _) = status_of(
        &c,
        format!("http://{addr}/api/app/update/install"),
        json!({"version": "9.9.9"}),
    )
    .await;
    assert_eq!(code, 202);
    assert_eq!(seen.lock().unwrap().last().unwrap(), "install 9.9.9");

    // Where the install cannot replace itself, it is refused with where to go.
    devplane::update::set(devplane::update::UpdateState::CannotReplace {
        version: "9.9.9".into(),
        why: "installed by the system's package manager, which updates it".into(),
        where_to_get: devplane::update::RELEASES.into(),
    });
    let v = get(&c, &addr, "/api/app/update").await;
    assert_eq!(v["where"], devplane::update::RELEASES);
    let (code, v) = status_of(
        &c,
        format!("http://{addr}/api/app/update/install"),
        json!({"version": "9.9.9", "confirm": true}),
    )
    .await;
    assert_eq!((code, v["refused"].as_str()), (409, Some("cannot_replace")));

    std::fs::remove_dir_all(&home).ok();
}
