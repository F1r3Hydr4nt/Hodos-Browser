//! Local SV-node JSON-RPC broadcaster (ChainBackend::LocalNode).
//!
//! D-2a (BOLT Layer D). Broadcasts via `sendrawtransaction` and mines regtest
//! blocks via `generatetoaddress`, the same shape bolt-wallet uses. The HTTP
//! call is a thin reqwest wrapper; the request-building and response-parsing
//! (where bugs live) are pure functions, unit-tested against recorded node
//! responses.

use serde_json::{json, Value};

#[derive(Debug, PartialEq)]
pub enum NodeError {
    /// JSON-RPC returned an error object (node rejected the call).
    Rpc { code: i64, message: String },
    /// Response body was not the expected shape.
    Parse(String),
    /// Transport/HTTP failure.
    Http(String),
}

impl std::fmt::Display for NodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeError::Rpc { code, message } => write!(f, "node rpc error {code}: {message}"),
            NodeError::Parse(m) => write!(f, "node response parse error: {m}"),
            NodeError::Http(m) => write!(f, "node http error: {m}"),
        }
    }
}
impl std::error::Error for NodeError {}

/// Build a Bitcoin JSON-RPC 1.0 request body.
pub fn build_rpc_body(method: &str, params: Vec<Value>) -> String {
    json!({
        "jsonrpc": "1.0",
        "id": "hodos",
        "method": method,
        "params": params,
    })
    .to_string()
}

/// Parse a JSON-RPC response body into the `result` value, or a `NodeError`.
/// Works whether the node returned the error with HTTP 200 or 500.
pub fn parse_rpc_response(body: &str) -> Result<Value, NodeError> {
    let v: Value = serde_json::from_str(body)
        .map_err(|e| NodeError::Parse(format!("invalid json: {e}")))?;
    if let Some(err) = v.get("error") {
        if !err.is_null() {
            let code = err.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            let message = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("")
                .to_string();
            return Err(NodeError::Rpc { code, message });
        }
    }
    v.get("result")
        .cloned()
        .ok_or_else(|| NodeError::Parse("missing result field".into()))
}

/// Extract a txid (string `result`) from a parsed response.
pub fn parse_txid(body: &str) -> Result<String, NodeError> {
    let result = parse_rpc_response(body)?;
    result
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| NodeError::Parse("result is not a txid string".into()))
}

/// Extract mined block hashes (array `result`) from a parsed response.
pub fn parse_block_hashes(body: &str) -> Result<Vec<String>, NodeError> {
    let result = parse_rpc_response(body)?;
    let arr = result
        .as_array()
        .ok_or_else(|| NodeError::Parse("result is not an array".into()))?;
    arr.iter()
        .map(|h| {
            h.as_str()
                .map(|s| s.to_string())
                .ok_or_else(|| NodeError::Parse("block hash is not a string".into()))
        })
        .collect()
}

/// Mined-confirmation info from `getrawtransaction <txid> true` (verbose).
#[derive(Debug, PartialEq)]
pub struct TxConfirmation {
    pub confirmations: i64,
    pub blockhash: Option<String>,
}

impl TxConfirmation {
    pub fn is_confirmed(&self) -> bool {
        self.confirmations > 0
    }
}

/// Parse the verbose `getrawtransaction` response into confirmation info.
/// An unconfirmed (mempool) tx has no `confirmations`/`blockhash`.
pub fn parse_tx_confirmation(body: &str) -> Result<TxConfirmation, NodeError> {
    let result = parse_rpc_response(body)?;
    Ok(TxConfirmation {
        confirmations: result.get("confirmations").and_then(|c| c.as_i64()).unwrap_or(0),
        blockhash: result
            .get("blockhash")
            .and_then(|b| b.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string()),
    })
}

/// Thin async JSON-RPC client over reqwest with HTTP basic auth.
pub struct NodeRpc {
    client: reqwest::Client,
    url: String,
    user: String,
    pass: String,
}

impl NodeRpc {
    pub fn new(url: impl Into<String>, user: impl Into<String>, pass: impl Into<String>) -> Self {
        NodeRpc {
            client: reqwest::Client::new(),
            url: url.into(),
            user: user.into(),
            pass: pass.into(),
        }
    }

    async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value, NodeError> {
        let body = build_rpc_body(method, params);
        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| NodeError::Http(e.to_string()))?;
        let text = resp.text().await.map_err(|e| NodeError::Http(e.to_string()))?;
        parse_rpc_response(&text)
    }

    /// Broadcast a raw transaction; returns its txid.
    pub async fn send_raw_transaction(&self, raw_hex: &str) -> Result<String, NodeError> {
        let result = self
            .call("sendrawtransaction", vec![json!(raw_hex)])
            .await?;
        result
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| NodeError::Parse("sendrawtransaction result is not a txid".into()))
    }

    /// Mine `nblocks` regtest blocks to `address`; returns the block hashes.
    pub async fn generate_to_address(
        &self,
        nblocks: u32,
        address: &str,
    ) -> Result<Vec<String>, NodeError> {
        let result = self
            .call("generatetoaddress", vec![json!(nblocks), json!(address)])
            .await?;
        let arr = result
            .as_array()
            .ok_or_else(|| NodeError::Parse("generatetoaddress result is not an array".into()))?;
        arr.iter()
            .map(|h| {
                h.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| NodeError::Parse("block hash is not a string".into()))
            })
            .collect()
    }

    /// Fetch verbose tx info to determine mined confirmation status.
    pub async fn get_tx_confirmation(&self, txid: &str) -> Result<TxConfirmation, NodeError> {
        let body = build_rpc_body("getrawtransaction", vec![json!(txid), json!(true)]);
        let resp = self
            .client
            .post(&self.url)
            .basic_auth(&self.user, Some(&self.pass))
            .header("content-type", "application/json")
            .body(body)
            .send()
            .await
            .map_err(|e| NodeError::Http(e.to_string()))?;
        let text = resp.text().await.map_err(|e| NodeError::Http(e.to_string()))?;
        parse_tx_confirmation(&text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_body_shapes_a_bitcoin_jsonrpc_request() {
        let body = build_rpc_body("sendrawtransaction", vec![json!("deadbeef")]);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(v["jsonrpc"], "1.0");
        assert_eq!(v["method"], "sendrawtransaction");
        assert_eq!(v["params"][0], "deadbeef");
        assert_eq!(v["id"], "hodos");
    }

    #[test]
    fn parse_sendrawtransaction_success() {
        // recorded SV-node response
        let body = r#"{"result":"a1b2c3d4e5","error":null,"id":"hodos"}"#;
        assert_eq!(parse_txid(body).unwrap(), "a1b2c3d4e5");
    }

    #[test]
    fn parse_rpc_error_maps_to_node_error() {
        // recorded reject (non-standard script under standard policy, code -26)
        let body = r#"{"result":null,"error":{"code":-26,"message":"scriptpubkey"},"id":"hodos"}"#;
        match parse_rpc_response(body) {
            Err(NodeError::Rpc { code, message }) => {
                assert_eq!(code, -26);
                assert_eq!(message, "scriptpubkey");
            }
            other => panic!("expected Rpc error, got {other:?}"),
        }
    }

    #[test]
    fn parse_generatetoaddress_block_hashes() {
        let body = r#"{"result":["0000aaa","0000bbb"],"error":null,"id":"hodos"}"#;
        assert_eq!(parse_block_hashes(body).unwrap(), vec!["0000aaa", "0000bbb"]);
    }

    #[test]
    fn parse_rejects_non_txid_result() {
        let body = r#"{"result":{"unexpected":true},"error":null,"id":"hodos"}"#;
        assert!(matches!(parse_txid(body), Err(NodeError::Parse(_))));
    }

    #[test]
    fn parse_rejects_invalid_json() {
        assert!(matches!(parse_rpc_response("not json"), Err(NodeError::Parse(_))));
    }

    #[test]
    fn confirmation_confirmed() {
        let body = r#"{"result":{"txid":"abc","confirmations":6,"blockhash":"0000dead"},"error":null,"id":"hodos"}"#;
        let c = parse_tx_confirmation(body).unwrap();
        assert_eq!(c.confirmations, 6);
        assert_eq!(c.blockhash.as_deref(), Some("0000dead"));
        assert!(c.is_confirmed());
    }

    #[test]
    fn confirmation_mempool_only() {
        // unconfirmed: node omits confirmations/blockhash
        let body = r#"{"result":{"txid":"abc"},"error":null,"id":"hodos"}"#;
        let c = parse_tx_confirmation(body).unwrap();
        assert_eq!(c.confirmations, 0);
        assert_eq!(c.blockhash, None);
        assert!(!c.is_confirmed());
    }

    #[test]
    fn confirmation_propagates_rpc_error() {
        let body = r#"{"result":null,"error":{"code":-5,"message":"No such mempool tx"},"id":"hodos"}"#;
        assert!(matches!(parse_tx_confirmation(body), Err(NodeError::Rpc { code: -5, .. })));
    }
}
