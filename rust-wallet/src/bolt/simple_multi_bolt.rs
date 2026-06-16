//! SimpleMultiBolt (SMB) builder - Rust port (Layer B, B-4).
//!
//! Fungible token (16-byte balance) - the demo's £IRP loan currency (B3/B5).
//! 11 lockArgs; genesis mint sets balance + issuer, everything else zero, with
//! txoType = 0x20 (settle) and outputIndexN = 0x00. Mint-lock derivation reuses
//! the proven pattern; transfer/split/merge unlock (getAncestorPieceFungible) is
//! the heavier follow-on.

use std::collections::HashMap;

use super::lib::hash160;

/// Genesis mint lockArgs for a self-issued SimpleMultiBolt owned by `owner_pubkey`
/// carrying immutable-at-genesis 16-byte `balance`.
pub fn mint_lock_args(owner_pubkey: &[u8], balance: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    m.insert("balance".to_string(), balance.to_vec());
    m.insert("balanceCommit".to_string(), vec![0u8; 16]);
    m.insert("pubKeyHash".to_string(), hash160(owner_pubkey).to_vec());
    m.insert("pubKeyHashCommit".to_string(), vec![0u8; 20]);
    m.insert("pubKeyHashCommit2".to_string(), vec![0u8; 20]);
    m.insert("otherGrandparentOutpoint".to_string(), vec![0u8; 36]);
    m.insert("txoType".to_string(), vec![0x20]); // settle (genesis)
    m.insert("outputIndexN".to_string(), vec![0x00]);
    m.insert("parentOutpoint".to_string(), vec![0u8; 36]);
    m.insert("grandparentOutpoint".to_string(), vec![0u8; 36]);
    m.insert("issuerPubKey".to_string(), owner_pubkey.to_vec());
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::sx_template::{fill_locking_script, simple_multi_bolt};
    use serde_json::Value;

    /// B-4 mint-lock golden: deriving the 11 genesis args (incl. the 16-byte
    /// balance) and filling reproduces the SxSimulator's SMB tx0 bolt output lock
    /// byte-for-byte - proving the engine generalizes to the fungible contract.
    #[test]
    fn mint_lock_matches_lifecycle_fixture_tx0() {
        const OWNER: &str = "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";
        let owner = hex::decode(OWNER).unwrap();
        let balance = hex::decode("feffffffffff1f000000000000000000").unwrap();

        let args = mint_lock_args(&owner, &balance);
        let lock = fill_locking_script(&simple_multi_bolt(), &args).expect("fill");

        const LIFE: &str = include_str!("../../tests/fixtures/simplemultibolt_lifecycle.json");
        let fx: Value = serde_json::from_str(LIFE).unwrap();
        let expected = fx["txs"][0]["outs"][0]["lockHex"].as_str().unwrap();

        assert_eq!(hex::encode(&lock), expected, "SimpleMultiBolt mint lock mismatch vs fixture tx0");
    }
}
