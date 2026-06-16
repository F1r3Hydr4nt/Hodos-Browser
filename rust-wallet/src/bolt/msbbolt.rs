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
    use crate::bolt::ctx::{ctx_footer, ctx_header, hash_outputs, hash_prevouts, hash_sequence};
    use crate::bolt::min_simple_bolt::{melt_unlock_args, simple_spend_unlock_args};
    use crate::bolt::sx_template::{fill_locking_script, fill_unlocking_script, min_simple_balance_bolt};
    use serde_json::Value;

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

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

    /// B-3 transfer: MSBBolt's unlockArgs are identical to MinSimpleBolt's, so the
    /// SAME simple_spend_unlock_args reproduces MSBBolt's tx1 transfer unlock
    /// byte-for-byte (just passing MSBBolt's 1296B spent lock + artifact). Proves
    /// the unlock builder is contract-generic across NFT variants.
    #[test]
    fn transfer_unlock_matches_fixture() {
        const TX1: &str = include_str!("../../tests/fixtures/msbbolt_spend_tx1.json");
        let fx: Value = serde_json::from_str(TX1).unwrap();
        let st = &fx["struct"];
        let ua = fx["args"].as_object().unwrap();

        let owner = dehex(&ua["pubKey"]);
        let spent_lock = dehex(&st["spentLockScript"]);

        let inputs = st["inputs"].as_array().unwrap();
        let outpoints: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["outpoint"])).collect();
        let sequences: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["sequence"])).collect();
        let outputs_raw: Vec<(Vec<u8>, Vec<u8>)> = st["outputs"].as_array().unwrap().iter()
            .map(|o| (dehex(&o["value"]), dehex(&o["script"]))).collect();
        let bolt_idx = st["boltInputIndex"].as_u64().unwrap() as usize;

        let header = ctx_header(&dehex(&st["version"]), &hash_prevouts(&outpoints),
            &hash_sequence(&sequences), &outpoints[bolt_idx]);
        let footer = ctx_footer(&dehex(&st["spentValue"]), &sequences[bolt_idx],
            &hash_outputs(&outputs_raw), &dehex(&st["locktime"]), &dehex(&st["sighashType"]));

        let fund_outpoint = outpoints[1 - bolt_idx].clone();

        let args = simple_spend_unlock_args(
            &owner, &spent_lock, &fund_outpoint, &dehex(&ua["changeOutput"]),
            &dehex(&ua["beneficiaryPubKeyHash"]), &header, &footer, &dehex(&ua["sig"]),
        );
        let built = fill_unlocking_script(&min_simple_balance_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "MSBBolt tx1 transfer unlock mismatch");
    }

    /// B-3 melt: reuses melt_unlock_args (only sig+pubKey) + the MSBBolt artifact.
    #[test]
    fn melt_unlock_matches_fixture() {
        const TX5: &str = include_str!("../../tests/fixtures/msbbolt_spend_tx5.json");
        let fx: Value = serde_json::from_str(TX5).unwrap();
        let ua = fx["args"].as_object().unwrap();
        let args = melt_unlock_args(&dehex(&ua["pubKey"]), &dehex(&ua["sig"]));
        let built = fill_unlocking_script(&min_simple_balance_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "MSBBolt melt unlock mismatch");
    }
}
