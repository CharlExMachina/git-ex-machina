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

/// A download can take minutes on a slow line, so it has no overall limit; a connection that
/// goes quiet for a minute fails instead of hanging.
fn download_agent() -> &'static ureq::Agent {
    static AGENT: OnceLock<ureq::Agent> = OnceLock::new();
    AGENT.get_or_init(|| {
        let tls = ureq::tls::TlsConfig::builder().root_certs(ureq::tls::RootCerts::PlatformVerifier).build();
        ureq::Agent::config_builder()
            .tls_config(tls)
            .timeout_connect(Some(Duration::from_secs(30)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_per_read(Some(Duration::from_secs(60)))
            .user_agent("git-ex-machina")
            .build()
            .into()
    })
}

/// GETs `url` without a token, as GitHub's REST API answers anyone about a public repository,
/// and reads the JSON answer.
pub fn get_json(url: &str) -> Result<Value, ForgeError> {
    read(agent().get(url).header("Accept", "application/vnd.github+json").call())
}

/// GETs `url` (following redirects) into `out`, telling `progress` the bytes so far and the
/// total the server announced.
pub fn download(
    url: &str,
    out: &mut dyn std::io::Write,
    progress: &mut dyn FnMut(u64, Option<u64>),
) -> Result<(), ForgeError> {
    let mut response = match download_agent().get(url).header("Accept", "application/octet-stream").call() {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => return Err(ForgeError::Status(code)),
        Err(e) => return Err(ForgeError::Network(e.to_string())),
    };
    let total = response.body().content_length();
    let mut body = response.body_mut().as_reader();
    let mut buf = vec![0; 64 * 1024];
    let mut done = 0u64;
    progress(0, total);
    loop {
        let n = std::io::Read::read(&mut body, &mut buf).map_err(|e| ForgeError::Network(e.to_string()))?;
        if n == 0 {
            return Ok(());
        }
        out.write_all(&buf[..n]).map_err(|e| ForgeError::Network(format!("couldn't write the download: {e}")))?;
        done += n as u64;
        progress(done, total);
    }
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

fn read(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, ForgeError> {
    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => return Err(ForgeError::Status(code)),
        Err(e) => return Err(ForgeError::Network(e.to_string())),
    };
    let text = response.body_mut().read_to_string().map_err(|e| ForgeError::Network(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| ForgeError::Parse(e.to_string()))
}
