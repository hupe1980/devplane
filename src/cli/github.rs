//! `devplane login github`, `devplane logout github`, and `doctor`'s GitHub
//! section. Sign-in goes through the running host when there is one (so the
//! window shows the same code) and runs in this process otherwise; both store
//! to the same credential-store entry.

use crate::github::{GitHub, Secret};
use crate::render::{BOLD, DIM, paint};
use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

/// Whether `url` may be handed to the platform's opener: a web address with
/// nothing a shell or a command line could read as more than one word.
fn openable(url: &str) -> bool {
    url.starts_with("https://")
        && url.len() < 2048
        && url.chars().all(|c| {
            c.is_ascii_graphic() && !matches!(c, '"' | '\'' | '`' | '<' | '>' | '|' | '^' | '\\')
        })
}

/// Opens an address in the browser, best effort: the address is printed
/// either way.
fn open_in_browser(url: &str) {
    // Only a web address: a device page is never anything else, and a
    // command line must not be handed a path or an option.
    if !openable(url) {
        return;
    }
    let to = url.to_string();
    #[cfg(target_os = "macos")]
    let cmd = std::process::Command::new("open").arg(&to).spawn();
    // Not `cmd /C start`: cmd would read `&` in the address as a second
    // command. Explorer takes it as one argument and hands it to the browser.
    #[cfg(target_os = "windows")]
    let cmd = std::process::Command::new("explorer.exe").arg(&to).spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let cmd = std::process::Command::new("xdg-open").arg(&to).spawn();
    let _ = cmd;
}

/// The first line `--json` prints: the code and where to enter it, before
/// the wait — a script that saw nothing until the end could not show it.
fn pending_line(host: &str, user_code: &str, uri: &str, expires_at: &str) -> Value {
    json!({"host": host, "state": "pending", "user_code": user_code,
           "verification_uri": uri, "expires_at": expires_at})
}

/// The code to enter: a JSON line under `--json`, the sentence otherwise.
fn print_code(host: &str, user_code: &str, uri: &str, expires_at: &str, json: bool) {
    if json {
        println!("{}", pending_line(host, user_code, uri, expires_at));
        return;
    }
    println!("Open {uri} and enter the code:\n");
    println!("    {}\n", paint(BOLD, user_code));
    let mins = expires_at
        .parse::<jiff::Timestamp>()
        .map(|t| ((t - jiff::Timestamp::now()).get_seconds().max(0) + 59) / 60)
        .unwrap_or(15);
    println!("waiting for GitHub… (the code expires in {mins} minutes)");
}

fn signed_in(host: &str, login: &str, scopes: &[String], store: &str, json: bool) {
    if json {
        println!(
            "{}",
            json!({"host": host, "login": login, "scopes": scopes})
        );
    } else {
        println!("signed in to {host} as {login}");
        println!("  {}", paint(DIM, &format!("the token is in {store}")));
    }
}

/// Reads a token from stdin, refusing a terminal with nothing piped: a token
/// typed as an argument would sit in the shell history.
fn token_from_stdin() -> Result<Secret> {
    use std::io::{IsTerminal, Read};
    if std::io::stdin().is_terminal() {
        bail!(
            "`--with-token` reads the token from stdin, never an argument: \
             `echo \"$TOKEN\" | devplane login github --with-token`"
        );
    }
    let mut s = String::new();
    std::io::stdin()
        .read_to_string(&mut s)
        .context("reading the token from stdin")?;
    let t = s.trim();
    if t.is_empty() {
        bail!("nothing was piped to `--with-token`");
    }
    Ok(Secret::new(t))
}

/// Tells a running host that this process changed a sign-in, so it drops
/// its copy of the token and reads the forge now. Best effort: without a
/// host there is nobody to tell.
async fn tell_the_host(host: &str) {
    if let Ok(c) = crate::client::Client::connect_running().await {
        let _: Result<Value> = c
            .post_json("/api/github/refresh", &json!({"host": host}))
            .await;
    }
}

pub async fn cmd_login(host: Option<String>, with_token: bool, json: bool) -> Result<()> {
    let hub = GitHub::for_home()?;
    let host = hub.host_or_default(host.as_deref());

    if with_token {
        let token = token_from_stdin()?;
        let v = hub.finish(&host, token).await?;
        tell_the_host(&host).await;
        signed_in(&host, &v.login, &v.scopes, &hub.describe_store(&host), json);
        return Ok(());
    }

    // Through the host when one runs: the window shows the same code.
    if let Ok(c) = crate::client::Client::connect_running().await {
        let started: Value = c
            .post_json("/api/github/login", &json!({"host": host}))
            .await?;
        if let Some(e) = started["error"].as_str() {
            bail!("{e}");
        }
        let uri = started["verification_uri"].as_str().unwrap_or_default();
        print_code(
            &host,
            started["user_code"].as_str().unwrap_or_default(),
            uri,
            started["expires_at"].as_str().unwrap_or_default(),
            json,
        );
        open_in_browser(uri);
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            let all: Value = c.get("/api/github").await?;
            let Some(now) = all["hosts"]
                .as_array()
                .and_then(|h| h.iter().find(|v| v["host"] == host.as_str()))
            else {
                continue;
            };
            if now["state"] == "pending" {
                continue;
            }
            // Said first: a denied attempt while an older sign-in stands
            // leaves the host signed in, and is still a denial.
            if let Some(said) = now["said"].as_str() {
                bail!("{said}");
            }
            if now["state"] == "signed_in" {
                let scopes: Vec<String> =
                    serde_json::from_value(now["scopes"].clone()).unwrap_or_default();
                signed_in(
                    &host,
                    now["login"].as_str().unwrap_or_default(),
                    &scopes,
                    &hub.describe_store(&host),
                    json,
                );
                return Ok(());
            }
            bail!("the sign-in ended without a token");
        }
    }

    let pending = hub.start(&host).await?;
    print_code(
        &host,
        &pending.user_code,
        &pending.verification_uri,
        &pending.expires_at.to_string(),
        json,
    );
    open_in_browser(&pending.verification_uri);
    let v = hub.wait(pending).await?;
    signed_in(&host, &v.login, &v.scopes, &hub.describe_store(&host), json);
    Ok(())
}

pub async fn cmd_logout(host: Option<String>, json: bool) -> Result<()> {
    let hub = GitHub::for_home()?;
    let host = hub.host_or_default(host.as_deref());
    let from = match crate::client::Client::connect_running().await {
        Ok(c) => {
            let v: Value = c
                .post_json("/api/github/logout", &json!({"host": host}))
                .await?;
            if let Some(e) = v["error"].as_str() {
                bail!("{e}");
            }
            v["deleted_from"].as_str().map(str::to_string)
        }
        Err(_) => hub.logout(&host).await?,
    };
    let revoke = crate::github::revoke_url(&host);
    if json {
        println!(
            "{}",
            json!({"host": host, "state": "signed_out", "deleted_from": from, "revoke_at": revoke})
        );
        return Ok(());
    }
    match from {
        Some(from) => {
            println!("signed out of {host}: the token is deleted from {from}");
            println!(
                "  {}",
                paint(
                    DIM,
                    &format!("the grant still exists at GitHub; revoke it at {revoke}")
                )
            );
        }
        None => println!("not signed in to {host}; there was no token to delete"),
    }
    Ok(())
}

/// One line per GitHub host: whom it signs in as, the scopes, when GitHub
/// last answered. From the host's diagnostics when one runs, else from this
/// machine's sign-in record. Never the token.
pub fn doctor_lines(hosts: &Value, default_host: &str) -> Vec<String> {
    let empty = vec![];
    let hosts = hosts.as_array().unwrap_or(&empty);
    let width = hosts
        .iter()
        .filter_map(|h| h["host"].as_str().map(str::len))
        .max()
        .unwrap_or(10)
        .max(10);
    hosts
        .iter()
        .map(|h| {
            let name = h["host"].as_str().unwrap_or_default();
            let flag = if name == default_host {
                String::new()
            } else {
                format!(" --host {name}")
            };
            let says = match h["state"].as_str().unwrap_or_default() {
                "signed_in" => {
                    let scopes: Vec<&str> = h["scopes"]
                        .as_array()
                        .map(|s| s.iter().filter_map(|x| x.as_str()).collect())
                        .unwrap_or_default();
                    let read = h["last_read"]
                        .as_str()
                        .and_then(|t| t.parse::<jiff::Timestamp>().ok())
                        .map(|t| format!("last read {} ago", ago(t)))
                        .unwrap_or_else(|| "not read yet".into());
                    format!(
                        "signed in as {} · {} · {read}",
                        h["login"].as_str().unwrap_or("?"),
                        if scopes.is_empty() {
                            "scopes unknown".to_string()
                        } else {
                            scopes.join(", ")
                        }
                    )
                }
                "pending" => format!(
                    "signing in — enter {} at {}",
                    h["user_code"].as_str().unwrap_or("?"),
                    h["verification_uri"].as_str().unwrap_or("?")
                ),
                "expired" => format!("sign-in expired — devplane login github{flag}"),
                "rate_limited" => {
                    format!("rate limited until {}", h["until"].as_str().unwrap_or("?"))
                }
                "unreachable" => format!(
                    "unreachable since {} — {}",
                    h["since"].as_str().unwrap_or("?"),
                    h["why"].as_str().unwrap_or("")
                ),
                _ => format!("not signed in — devplane login github{flag}"),
            };
            format!("  {name:<width$}   {says}")
        })
        .collect()
}

fn ago(t: jiff::Timestamp) -> String {
    let s = (jiff::Timestamp::now() - t).get_seconds().max(0);
    match s {
        s if s < 90 => format!("{s}s"),
        s if s < 5400 => format!("{}m", s / 60),
        s => format!("{}h", s / 3600),
    }
}

/// Every GitHub host's sign-in and the configured host: from the running
/// host's diagnostics when there are some, else from this machine's record.
pub fn doctor_hosts(diag: Option<&Value>) -> (Value, String) {
    match (diag.map(|d| &d["forge"]["github"]), GitHub::for_home().ok()) {
        (Some(v), Some(hub)) if v.is_array() => (v.clone(), hub.default_host().to_string()),
        (_, Some(hub)) => (
            serde_json::to_value(hub.views()).unwrap_or_default(),
            hub.default_host().to_string(),
        ),
        (_, None) => (Value::Null, "github.com".to_string()),
    }
}

/// `doctor`'s GitHub section.
pub fn print_doctor(diag: Option<&Value>) {
    println!("\n{}", paint(BOLD, "github"));
    let (hosts, default_host) = doctor_hosts(diag);
    for line in doctor_lines(&hosts, &default_host) {
        println!("{line}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_says_the_code_before_it_waits() {
        let v = pending_line(
            "github.com",
            "WDJB-MJHT",
            "https://github.com/login/device",
            "t",
        );
        assert_eq!(v["state"], "pending");
        assert_eq!(v["user_code"], "WDJB-MJHT");
        assert_eq!(v["verification_uri"], "https://github.com/login/device");
    }

    #[test]
    fn only_a_plain_web_address_is_handed_to_the_opener() {
        assert!(openable("https://github.com/login/device"));
        assert!(openable("https://ghe.corp/login/device?x=1&y=2"));
        for bad in [
            "http://github.com/login/device",
            "https://github.com/login/device\" & calc",
            "https://evil.example/a b",
            "https://x.example/|calc",
            "https://x.example/^",
            "-oProxyCommand=calc",
            "file:///etc/passwd",
        ] {
            assert!(!openable(bad), "{bad}");
        }
    }

    #[test]
    fn the_doctor_section_names_each_host_and_never_a_token() {
        let hosts = json!([
            {"host": "github.com", "state": "signed_in", "login": "octocat",
             "scopes": ["repo", "read:org"], "last_read": jiff::Timestamp::now().to_string()},
            {"host": "ghe.corp", "state": "signed_out", "last_read": null},
        ]);
        let lines = doctor_lines(&hosts, "github.com");
        assert!(lines[0].contains("signed in as octocat · repo, read:org · last read"));
        assert!(
            lines[1].contains("not signed in — devplane login github --host ghe.corp"),
            "{lines:?}"
        );
    }
}
