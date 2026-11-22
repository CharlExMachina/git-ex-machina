//! The network edge: JSON over HTTPS, verified by the platform's trust store.

use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;

use super::ForgeError;

/// One agent for the process, so connections are reused.
fn agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_global(Some(Duration::from_secs(30)))
            .user_agent("git-ex-machina")
            .build()
            .into()
    })
}

/// POSTs `body` as JSON with a bearer token, and reads the JSON answer.
pub fn post_json(url: &str, token: &str, body: &Value) -> Result<Value, ForgeError> {
    let response = agent()
        .post(url)
        .header("Authorization", format!("bearer {token}"))
        .header("Accept", "application/json")
        .content_type("application/json")
        .send(body.to_string());
    read(response)
}

/// POSTs `fields` as a form, as OAuth's sign-in endpoints take them, and reads the JSON answer.
pub fn post_form(url: &str, fields: &[(&str, &str)]) -> Result<Value, ForgeError> {
    read(agent().post(url).header("Accept", "application/json").send_form(fields.iter().copied()))
}

/// One REST call: `method` on `url` with a bearer token, and `body` as JSON when there is one.
/// A `204 No Content` reads as `Value::Null`. A refusal keeps its status for 401, 403 and 404
/// (what the caller acts on) and GitHub's own words for the rest (`Validation Failed: …`).
pub fn rest(method: &str, url: &str, token: &str, body: Option<&Value>) -> Result<Value, ForgeError> {
    let auth = format!("bearer {token}");
    let headers = |b: ureq::RequestBuilder<ureq::typestate::WithoutBody>| {
        b.header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .config()
            .http_status_as_error(false)
            .build()
    };
    let with_body = |b: ureq::RequestBuilder<ureq::typestate::WithBody>| {
        b.header("Authorization", &auth)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .config()
            .http_status_as_error(false)
            .build()
            .content_type("application/json")
            .send(body.map_or_else(|| "{}".to_owned(), Value::to_string))
    };
    let response = match method {
        "GET" => headers(agent().get(url)).call(),
        "DELETE" => headers(agent().delete(url)).call(),
        "POST" => with_body(agent().post(url)),
        "PUT" => with_body(agent().put(url)),
        "PATCH" => with_body(agent().patch(url)),
        other => return Err(ForgeError::Api(format!("no such method {other}"))),
    };
    let mut response = response.map_err(|e| ForgeError::Network(e.to_string()))?;
    let status = response.status().as_u16();
    let text = response.body_mut().read_to_string().map_err(|e| ForgeError::Network(e.to_string()))?;
    let answer = if text.trim().is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&text).map_err(|e| ForgeError::Parse(e.to_string()))?
    };
    match status {
        200..=299 => Ok(answer),
        401 | 403 | 404 => Err(ForgeError::Status(status)),
        _ => Err(ForgeError::Api(said(&answer).unwrap_or_else(|| format!("it answered {status}")))),
    }
}

/// GitHub's reason for a refusal: its `message`, with the first of its `errors` when it gives
/// one (`Validation Failed: name already exists on this account`).
fn said(answer: &Value) -> Option<String> {
    let message = answer["message"].as_str()?;
    let detail = answer["errors"].as_array().and_then(|e| e.first()).and_then(|e| {
        e["message"].as_str().map(str::to_owned).or_else(|| {
            let (field, code) = (e["field"].as_str()?, e["code"].as_str()?);
            Some(format!("{field} {}", code.replace('_', " ")))
        })
    });
    Some(match detail {
        Some(d) => format!("{message}: {d}"),
        None => message.to_owned(),
    })
}

fn read(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, ForgeError> {
    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => return Err(ForgeError::Status(code)),
        Err(e) => return Err(ForgeError::Network(e.to_string())),
    };
    let text = response.body_mut().read_to_string().map_err(|e| ForgeError::Network(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| ForgeError::Parse(e.to_string()))
}
