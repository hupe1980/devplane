//! One GitHub host, spoken to over its documented GraphQL and REST APIs.
//!
//! Every request carries the headers `contracts/github.md` names; every
//! failure is one of the [`Error`](super::Error) states a surface can say,
//! never retried in a loop here.

use super::Error;
use super::auth::Secret;
use jiff::Timestamp;
use serde_json::Value;

/// Where one GitHub host answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHubHost {
    /// `github.com` or an Enterprise hostname — the key the token is stored
    /// under and the host a remote names.
    pub name: String,
    /// REST base, no trailing slash.
    pub api: String,
    pub graphql: String,
    /// Where `/login/device/code` and `/login/oauth/access_token` live.
    pub oauth: String,
}

impl GitHubHost {
    /// The documented addresses: `api.github.com` for github.com, `/api/v3`
    /// and `/api/graphql` on an Enterprise server.
    pub fn of(name: &str) -> Self {
        let name = name.trim().trim_end_matches('/').to_ascii_lowercase();
        if name == "github.com" {
            return Self {
                name,
                api: "https://api.github.com".into(),
                graphql: "https://api.github.com/graphql".into(),
                oauth: "https://github.com".into(),
            };
        }
        Self {
            api: format!("https://{name}/api/v3"),
            graphql: format!("https://{name}/api/graphql"),
            oauth: format!("https://{name}"),
            name,
        }
    }
}

/// The HTTP client every GitHub request goes through: TLS by rustls with the
/// OS's trust store, a bounded timeout, and Devplane's name and version.
pub(crate) fn http() -> reqwest::Client {
    static CLIENT: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    CLIENT
        .get_or_init(|| {
            reqwest::Client::builder()
                .user_agent(concat!("devplane/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(std::time::Duration::from_secs(10))
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new())
        })
        .clone()
}

/// A signed-in conversation with one host.
pub struct Client {
    host: GitHubHost,
    token: Secret,
    http: reqwest::Client,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("host", &self.host.name)
            .field("token", &self.token)
            .finish()
    }
}

/// A REST answer: the body and what the headers said about the token.
#[derive(Debug)]
pub struct Answer {
    pub body: Value,
    /// `X-OAuth-Scopes`, split.
    pub scopes: Vec<String>,
}

impl Client {
    pub fn new(host: GitHubHost, token: Secret) -> Self {
        Self {
            host,
            token,
            http: http(),
        }
    }

    fn request(&self, method: reqwest::Method, url: &str) -> reqwest::RequestBuilder {
        self.http
            .request(method, url)
            .header(
                reqwest::header::AUTHORIZATION,
                format!("Bearer {}", self.token.expose()),
            )
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
    }

    /// Runs one GraphQL document and returns its `data`.
    pub async fn graphql(&self, query: &str, variables: Value) -> Result<Value, Error> {
        let resp = self
            .request(reqwest::Method::POST, &self.host.graphql)
            .json(&serde_json::json!({"query": query, "variables": variables}))
            .send()
            .await
            .map_err(unreachable)?;
        let (status, headers, body) = read(resp).await?;
        refuse(&self.host.name, status, &headers, &body)?;
        if let Some(errors) = body["errors"].as_array().filter(|e| !e.is_empty()) {
            let first = &errors[0];
            let kind = first["type"].as_str().unwrap_or_default();
            let says = first["message"].as_str().unwrap_or("an error").to_string();
            return Err(match kind {
                "RATE_LIMITED" => Error::RateLimited {
                    until: reset_of(&headers).unwrap_or_else(|| in_seconds(60)),
                },
                "NOT_FOUND" => Error::NotFound(says),
                "FORBIDDEN" | "INSUFFICIENT_SCOPES" => Error::Forbidden(says),
                _ => Error::Api {
                    status: status.as_u16(),
                    message: says,
                },
            });
        }
        Ok(body["data"].clone())
    }

    /// One REST call; `path` starts with `/`.
    pub async fn rest(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<Value>,
    ) -> Result<Answer, Error> {
        let mut req = self.request(method, &format!("{}{path}", self.host.api));
        if let Some(b) = body {
            req = req.json(&b);
        }
        let resp = req.send().await.map_err(unreachable)?;
        let (status, headers, body) = read(resp).await?;
        refuse(&self.host.name, status, &headers, &body)?;
        let scopes = headers
            .get("x-oauth-scopes")
            .and_then(|v| v.to_str().ok())
            .map(|s| {
                s.split(',')
                    .map(|p| p.trim().to_string())
                    .filter(|p| !p.is_empty())
                    .collect()
            })
            .unwrap_or_default();
        Ok(Answer { body, scopes })
    }
}

fn unreachable(e: reqwest::Error) -> Error {
    // Never the URL's query or a header: only what failed.
    let why = if e.is_timeout() {
        "timed out".to_string()
    } else if e.is_connect() {
        "could not connect".to_string()
    } else {
        e.without_url().to_string()
    };
    Error::Unreachable(why)
}

async fn read(
    resp: reqwest::Response,
) -> Result<(reqwest::StatusCode, reqwest::header::HeaderMap, Value), Error> {
    let status = resp.status();
    let headers = resp.headers().clone();
    let text = resp.text().await.map_err(unreachable)?;
    let body = serde_json::from_str(&text).unwrap_or(Value::Null);
    Ok((status, headers, body))
}

fn in_seconds(s: i64) -> Timestamp {
    Timestamp::now() + jiff::SignedDuration::from_secs(s)
}

/// When GitHub said the limit resets: `retry-after` (seconds), else
/// `x-ratelimit-reset` (epoch seconds).
fn reset_of(headers: &reqwest::header::HeaderMap) -> Option<Timestamp> {
    let h = |k: &str| headers.get(k).and_then(|v| v.to_str().ok());
    if let Some(s) = h("retry-after").and_then(|v| v.trim().parse::<i64>().ok()) {
        return Some(in_seconds(s.max(1)));
    }
    h("x-ratelimit-reset")
        .and_then(|v| v.trim().parse::<i64>().ok())
        .and_then(|s| Timestamp::from_second(s).ok())
}

/// GitHub's `message`, and each `errors[].message` after it: a 422 says
/// *Validation Failed* at the top and why only in the list.
fn says_of(body: &Value) -> String {
    let mut says = body["message"].as_str().unwrap_or("").to_string();
    let details: Vec<&str> = body["errors"]
        .as_array()
        .map(|e| e.iter().filter_map(|x| x["message"].as_str()).collect())
        .unwrap_or_default();
    if !details.is_empty() {
        if !says.is_empty() {
            says.push_str(": ");
        }
        says.push_str(&details.join("; "));
    }
    says
}

/// A non-success status, as the state it is.
fn refuse(
    host: &str,
    status: reqwest::StatusCode,
    headers: &reqwest::header::HeaderMap,
    body: &Value,
) -> Result<(), Error> {
    if status.is_success() {
        return Ok(());
    }
    let says = says_of(body);
    // A secondary limit may carry neither header; GitHub says so in the body.
    let limited = headers
        .get("x-ratelimit-remaining")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.trim() == "0")
        || headers.contains_key("retry-after")
        || says.to_ascii_lowercase().contains("secondary rate limit");
    match status.as_u16() {
        401 => Err(Error::Expired(host.to_string())),
        403 | 429 if limited => Err(Error::RateLimited {
            until: reset_of(headers).unwrap_or_else(|| in_seconds(60)),
        }),
        403 => {
            // Name what the token would need, when GitHub says.
            let needs = headers
                .get("x-accepted-oauth-scopes")
                .and_then(|v| v.to_str().ok())
                .filter(|v| !v.trim().is_empty())
                .map(|v| format!(" (it needs: {})", v.trim()))
                .unwrap_or_default();
            Err(Error::Forbidden(format!(
                "{}{needs}",
                if says.is_empty() {
                    "permission denied"
                } else {
                    &says
                }
            )))
        }
        404 => Err(Error::NotFound(if says.is_empty() {
            "not found".into()
        } else {
            says
        })),
        429 => Err(Error::RateLimited {
            until: in_seconds(60),
        }),
        code => Err(Error::Api {
            status: code,
            message: says,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_com_and_an_enterprise_server_have_their_documented_addresses() {
        let g = GitHubHost::of("GitHub.com");
        assert_eq!(g.api, "https://api.github.com");
        assert_eq!(g.graphql, "https://api.github.com/graphql");
        assert_eq!(g.oauth, "https://github.com");
        let e = GitHubHost::of("ghe.corp");
        assert_eq!(e.api, "https://ghe.corp/api/v3");
        assert_eq!(e.graphql, "https://ghe.corp/api/graphql");
        assert_eq!(e.oauth, "https://ghe.corp");
    }

    #[test]
    fn a_refusal_is_the_state_it_means() {
        let mut h = reqwest::header::HeaderMap::new();
        let none = Value::Null;
        assert_eq!(
            refuse("github.com", reqwest::StatusCode::UNAUTHORIZED, &h, &none),
            Err(Error::Expired("github.com".into()))
        );
        h.insert("x-ratelimit-remaining", "0".parse().unwrap());
        h.insert("x-ratelimit-reset", "1900000000".parse().unwrap());
        assert_eq!(
            refuse("github.com", reqwest::StatusCode::FORBIDDEN, &h, &none),
            Err(Error::RateLimited {
                until: Timestamp::from_second(1_900_000_000).unwrap()
            })
        );
        let plain = reqwest::header::HeaderMap::new();
        let body = serde_json::json!({"message": "Resource not accessible by integration"});
        assert!(matches!(
            refuse("github.com", reqwest::StatusCode::FORBIDDEN, &plain, &body),
            Err(Error::Forbidden(m)) if m.contains("not accessible")
        ));
    }

    #[test]
    fn a_secondary_limit_without_headers_is_a_limit() {
        let plain = reqwest::header::HeaderMap::new();
        let body = serde_json::json!({"message": "You have exceeded a secondary rate limit. Please wait a few minutes."});
        let before = Timestamp::now();
        match refuse("github.com", reqwest::StatusCode::FORBIDDEN, &plain, &body) {
            Err(Error::RateLimited { until }) => assert!(until > before),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_validation_failure_keeps_what_failed() {
        let plain = reqwest::header::HeaderMap::new();
        let body = serde_json::json!({"message": "Validation Failed", "errors": [
            {"resource": "PullRequest", "code": "custom",
             "message": "A pull request already exists for acme:feat/x."}
        ]});
        assert_eq!(
            refuse(
                "github.com",
                reqwest::StatusCode::UNPROCESSABLE_ENTITY,
                &plain,
                &body
            ),
            Err(Error::Api {
                status: 422,
                message: "Validation Failed: A pull request already exists for acme:feat/x.".into()
            })
        );
    }

    #[test]
    fn the_token_never_prints() {
        let c = Client::new(GitHubHost::of("github.com"), Secret::new("gho_supersecret"));
        assert!(!format!("{c:?}").contains("gho_supersecret"));
    }
}
