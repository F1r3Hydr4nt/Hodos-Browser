//! Arcade broadcaster (ChainBackend::Testnet / Ttn / Mainnet / LocalArcade).
//!
//! D-2b (BOLT Layer D). Submits a raw tx via Arcade `POST /tx`
//! (application/octet-stream) and classifies the response. As with node.rs the
//! HTTP call is a thin reqwest wrapper; the request-building and
//! response-classification are pure functions, unit-tested against the response
//! shapes documented in arcade/services/api_server/routes.go.

use serde_json::Value;

#[derive(Debug, PartialEq)]
pub enum SubmitStatus {
    /// 202 {"status":"submitted"} - accepted for propagation.
    Submitted,
    /// 202 {"status":"already submitted",...} - idempotent re-submit.
    AlreadySubmitted { txid: String, state: String },
}

#[derive(Debug, PartialEq)]
pub enum ArcadeError {
    /// 400 - validation failure (tx rejected, recorded REJECTED). Carries reason.
    Rejected(String),
    /// 503 - broker backpressure; safe to retry.
    Backpressure,
    /// Transport/HTTP failure.
    Http(String),
    /// Body not the expected shape.
    Parse(String),
    /// Any other status.
    Unexpected { status: u16, body: String },
}

/// Build the `POST /tx` URL for a raw-tx submission. Body is the raw tx bytes
/// sent as `application/octet-stream`.
pub fn build_submit_url(base_url: &str) -> String {
    format!("{}/tx", base_url.trim_end_matches('/'))
}

/// Classify an Arcade `POST /tx` response.
pub fn parse_submit_response(status: u16, body: &str) -> Result<SubmitStatus, ArcadeError> {
    match status {
        202 => {
            let v: Value = serde_json::from_str(body)
                .map_err(|e| ArcadeError::Parse(format!("invalid json: {e}")))?;
            match v.get("status").and_then(|s| s.as_str()) {
                Some("submitted") => Ok(SubmitStatus::Submitted),
                Some("already submitted") => Ok(SubmitStatus::AlreadySubmitted {
                    txid: v.get("txid").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                    state: v.get("state").and_then(|t| t.as_str()).unwrap_or("").to_string(),
                }),
                _ => Err(ArcadeError::Parse(format!("unexpected 202 body: {body}"))),
            }
        }
        400 => {
            // reason field per routes.go; fall back to other common keys.
            let reason = serde_json::from_str::<Value>(body)
                .ok()
                .and_then(|v| {
                    ["reason", "detail", "error", "message"]
                        .iter()
                        .find_map(|k| v.get(*k).and_then(|x| x.as_str()).map(|s| s.to_string()))
                })
                .unwrap_or_else(|| body.to_string());
            Err(ArcadeError::Rejected(reason))
        }
        503 => Err(ArcadeError::Backpressure),
        other => Err(ArcadeError::Unexpected { status: other, body: body.to_string() }),
    }
}

/// Thin async Arcade client.
pub struct ArcadeClient {
    client: reqwest::Client,
    base_url: String,
}

impl ArcadeClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        ArcadeClient { client: reqwest::Client::new(), base_url: base_url.into() }
    }

    /// Broadcast a raw transaction (octet-stream) and classify the result.
    pub async fn broadcast(&self, raw_tx: &[u8]) -> Result<SubmitStatus, ArcadeError> {
        let resp = self
            .client
            .post(build_submit_url(&self.base_url))
            .header("content-type", "application/octet-stream")
            .body(raw_tx.to_vec())
            .send()
            .await
            .map_err(|e| ArcadeError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.map_err(|e| ArcadeError::Http(e.to_string()))?;
        parse_submit_response(status, &text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submit_url_trims_trailing_slash() {
        assert_eq!(build_submit_url("https://arcade-v2-ttn-us-1.bsvblockchain.tech/"),
                   "https://arcade-v2-ttn-us-1.bsvblockchain.tech/tx");
        assert_eq!(build_submit_url("http://127.0.0.1:8080"), "http://127.0.0.1:8080/tx");
    }

    #[test]
    fn parse_submitted() {
        assert_eq!(parse_submit_response(202, r#"{"status":"submitted"}"#).unwrap(),
                   SubmitStatus::Submitted);
    }

    #[test]
    fn parse_already_submitted() {
        let body = r#"{"status":"already submitted","txid":"abc123","state":"SEEN_ON_NETWORK"}"#;
        assert_eq!(parse_submit_response(202, body).unwrap(),
                   SubmitStatus::AlreadySubmitted { txid: "abc123".into(), state: "SEEN_ON_NETWORK".into() });
    }

    #[test]
    fn parse_rejected_carries_reason() {
        let body = r#"{"reason":"mandatory-script-verify-flag-failed"}"#;
        assert_eq!(parse_submit_response(400, body),
                   Err(ArcadeError::Rejected("mandatory-script-verify-flag-failed".into())));
    }

    #[test]
    fn parse_backpressure() {
        assert_eq!(parse_submit_response(503, ""), Err(ArcadeError::Backpressure));
    }

    #[test]
    fn parse_unexpected_status() {
        match parse_submit_response(500, "boom") {
            Err(ArcadeError::Unexpected { status, body }) => {
                assert_eq!(status, 500);
                assert_eq!(body, "boom");
            }
            other => panic!("expected Unexpected, got {other:?}"),
        }
    }

    #[test]
    fn parse_rejects_bad_202_body() {
        assert!(matches!(parse_submit_response(202, "not json"), Err(ArcadeError::Parse(_))));
    }
}
