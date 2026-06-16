//! MinSimpleBolt (NFT base) builder - Rust port (Layer B, B-2).
//!
//! B-2 mint-lock half: derive the genesis mint's 6 lockArgs and fill the bolt
//! output locking script, byte-matching the SxSimulator fixture (tx0 out0).
//! The transfer commit/settle UNLOCK side (ancestor reconstruction) is B-2-unlock.

use std::collections::HashMap;

use super::lib::hash160;

/// Genesis mint lockArgs for a self-issued MinSimpleBolt owned by `owner_pubkey`
/// (33-byte compressed). Issuer == owner; commitment + parent/grandparent
/// outpoints are zero; txoType = 0x00 (genesis).
pub fn mint_lock_args(owner_pubkey: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    m.insert("pubKeyHash".to_string(), hash160(owner_pubkey).to_vec());
    m.insert("pubKeyHashCommitment".to_string(), vec![0u8; 20]);
    m.insert("txoType".to_string(), vec![0x00]);
    m.insert("parentOutpoint".to_string(), vec![0u8; 36]);
    m.insert("grandparentOutpoint".to_string(), vec![0u8; 36]);
    m.insert("issuerPubKey".to_string(), owner_pubkey.to_vec());
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::sx_template::{fill_locking_script, min_simple_bolt};
    use serde_json::Value;

    /// B-2 mint-lock golden: deriving the genesis args from the owner pubkey and
    /// filling reproduces the SxSimulator's tx0 bolt output lock byte-for-byte.
    #[test]
    fn mint_lock_matches_lifecycle_fixture_tx0() {
        const OWNER_PUBKEY: &str =
            "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";
        let owner = hex::decode(OWNER_PUBKEY).unwrap();

        let args = mint_lock_args(&owner);
        let lock = fill_locking_script(&min_simple_bolt(), &args).expect("fill");

        const LIFECYCLE: &str = include_str!("../../tests/fixtures/minsimplebolt_lifecycle.json");
        let fx: Value = serde_json::from_str(LIFECYCLE).unwrap();
        let expected = fx["txs"][0]["outs"][0]["lockHex"].as_str().unwrap();

        assert_eq!(hex::encode(&lock), expected, "MinSimpleBolt mint lock mismatch vs fixture tx0");
    }
}
