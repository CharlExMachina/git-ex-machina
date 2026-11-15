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

fn read(response: Result<ureq::http::Response<ureq::Body>, ureq::Error>) -> Result<Value, ForgeError> {
    let mut response = match response {
        Ok(r) => r,
        Err(ureq::Error::StatusCode(code)) => return Err(ForgeError::Status(code)),
        Err(e) => return Err(ForgeError::Network(e.to_string())),
    };
    let text = response.body_mut().read_to_string().map_err(|e| ForgeError::Network(e.to_string()))?;
    serde_json::from_str(&text).map_err(|e| ForgeError::Parse(e.to_string()))
}
