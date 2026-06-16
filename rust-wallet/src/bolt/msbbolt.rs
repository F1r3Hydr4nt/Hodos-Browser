//! MinimumSimpleBalanceBolt (MSBBolt) builder - Rust port (Layer B, B-3).
//!
//! NFT BOLT + immutable 16-byte balance (the demo's identity / proof-of-activity
//! token, B1/B2). Same genesis lock as MinSimpleBolt plus a leading `balance`
//! lockArg. Reuses the proven mint-lock derivation pattern; the unlock side
//! reuses the MinSimpleBolt simple-spend + ancestor-reconstruction algorithms.

use std::collections::HashMap;

use super::lib::hash160;

/// Genesis mint lockArgs for a self-issued MSBBolt owned by `owner_pubkey`
/// carrying immutable 16-byte `balance`.
pub fn mint_lock_args(owner_pubkey: &[u8], balance: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    m.insert("balance".to_string(), balance.to_vec());
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
    use crate::bolt::sx_template::{fill_locking_script, min_simple_balance_bolt};
    use serde_json::Value;

    /// B-3 mint-lock golden: deriving the genesis args (incl. the 16-byte balance)
    /// and filling reproduces the SxSimulator's MSBBolt tx0 bolt output lock
    /// byte-for-byte - proving the engine generalizes to a balance-carrying contract.
    #[test]
    fn mint_lock_matches_lifecycle_fixture_tx0() {
        const OWNER: &str = "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";
        let owner = hex::decode(OWNER).unwrap();
        let balance = hex::decode("000000000000000000000000000186a0").unwrap(); // 100000

        let args = mint_lock_args(&owner, &balance);
        let lock = fill_locking_script(&min_simple_balance_bolt(), &args).expect("fill");

        const LIFE: &str = include_str!("../../tests/fixtures/minsimplebalancebolt_lifecycle.json");
        let fx: Value = serde_json::from_str(LIFE).unwrap();
        let expected = fx["txs"][0]["outs"][0]["lockHex"].as_str().unwrap();

        assert_eq!(hex::encode(&lock), expected, "MSBBolt mint lock mismatch vs fixture tx0");
    }
}
