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
    /// 404 - Arcade has never seen this txid.
    NotFound,
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

/// Lifecycle state of a submitted tx (Arcade `GET /tx/:txid`). `merkle_path`
/// is the BUMP hex, present once mined (consumable by beef.rs).
#[derive(Debug, PartialEq)]
pub struct TxStatus {
    pub tx_status: String,
    pub block_hash: Option<String>,
    pub block_height: Option<u64>,
    pub merkle_path: Option<String>,
}

impl TxStatus {
    /// True once a BUMP merkle path is available (tx mined + proof built).
    pub fn is_mined(&self) -> bool {
        self.merkle_path.as_deref().map(|s| !s.is_empty()).unwrap_or(false)
    }
}

/// Classify an Arcade `GET /tx/:txid` response. 404 -> NotFound.
pub fn parse_tx_status(status: u16, body: &str) -> Result<TxStatus, ArcadeError> {
    match status {
        200 => {
            let v: Value = serde_json::from_str(body)
                .map_err(|e| ArcadeError::Parse(format!("invalid json: {e}")))?;
            let str_field = |k: &str| v.get(k).and_then(|x| x.as_str()).filter(|s| !s.is_empty()).map(|s| s.to_string());
            Ok(TxStatus {
                tx_status: v.get("txStatus").and_then(|x| x.as_str())
                    .or_else(|| v.get("status").and_then(|x| x.as_str()))
                    .unwrap_or("").to_string(),
                block_hash: str_field("blockHash"),
                block_height: v.get("blockHeight").and_then(|x| x.as_u64()),
                merkle_path: str_field("merklePath"),
            })
        }
        404 => Err(ArcadeError::NotFound),
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

    /// Look up a submitted tx's status (+ BUMP merkle path once mined).
    pub async fn get_tx_status(&self, txid: &str) -> Result<TxStatus, ArcadeError> {
        let url = format!("{}/tx/{}", self.base_url.trim_end_matches('/'), txid);
        let resp = self.client.get(url).send().await
            .map_err(|e| ArcadeError::Http(e.to_string()))?;
        let status = resp.status().as_u16();
        let text = resp.text().await.map_err(|e| ArcadeError::Http(e.to_string()))?;
        parse_tx_status(status, &text)
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

    #[test]
    fn tx_status_mined_exposes_bump() {
        let body = r#"{"txid":"abc","txStatus":"MINED","blockHash":"0000dead","blockHeight":870123,"merklePath":"fe..bump.."}"#;
        let s = parse_tx_status(200, body).unwrap();
        assert_eq!(s.tx_status, "MINED");
        assert_eq!(s.block_height, Some(870123));
        assert_eq!(s.block_hash.as_deref(), Some("0000dead"));
        assert_eq!(s.merkle_path.as_deref(), Some("fe..bump.."));
        assert!(s.is_mined());
    }

    #[test]
    fn tx_status_seen_but_not_mined() {
        let body = r#"{"txid":"abc","txStatus":"SEEN_ON_NETWORK","blockHash":null,"blockHeight":null,"merklePath":null}"#;
        let s = parse_tx_status(200, body).unwrap();
        assert_eq!(s.tx_status, "SEEN_ON_NETWORK");
        assert_eq!(s.merkle_path, None);
        assert_eq!(s.block_height, None);
        assert!(!s.is_mined());
    }

    #[test]
    fn tx_status_404_is_not_found() {
        assert_eq!(parse_tx_status(404, "{}"), Err(ArcadeError::NotFound));
    }
}
