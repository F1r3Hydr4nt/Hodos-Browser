//! MinSimpleDiscountBolt builder - Rust port (Layer B, B-5).
//!
//! NFT + immutable 1-byte percent discount - the demo's coupons (B4/B6/B7).
//! unlockArgs are IDENTICAL to MinSimpleBolt, so transfer/melt reuse those
//! builders directly; only the mint lock differs (a leading `discount` arg).

use std::collections::HashMap;

use super::lib::hash160;

/// Genesis mint lockArgs for a self-issued MinSimpleDiscountBolt owned by
/// `owner_pubkey` carrying an immutable 1-byte whole-percent `discount` (1..=100).
pub fn mint_lock_args(owner_pubkey: &[u8], discount: u8) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    m.insert("discount".to_string(), vec![discount]);
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
    use crate::bolt::sx_template::{fill_locking_script, fill_unlocking_script, min_simple_discount_bolt};
    use serde_json::Value;

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

    const OWNER: &str = "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";

    /// B-5 mint-lock golden: a 10% coupon mint reproduces the SxSimulator tx0 lock.
    #[test]
    fn mint_lock_matches_lifecycle_fixture_tx0() {
        let owner = hex::decode(OWNER).unwrap();
        let args = mint_lock_args(&owner, 0x0a); // 10%
        let lock = fill_locking_script(&min_simple_discount_bolt(), &args).expect("fill");

        const LIFE: &str = include_str!("../../tests/fixtures/minsimplediscountbolt_lifecycle.json");
        let fx: Value = serde_json::from_str(LIFE).unwrap();
        assert_eq!(hex::encode(&lock), fx["txs"][0]["outs"][0]["lockHex"].as_str().unwrap(), "Discount mint lock");
    }

    /// B-5 transfer: reuses MinSimpleBolt simple_spend_unlock_args (identical
    /// unlockArgs) + the Discount artifact; reproduces tx1's unlock byte-for-byte.
    #[test]
    fn transfer_unlock_matches_fixture() {
        const TX1: &str = include_str!("../../tests/fixtures/discount_spend_tx1.json");
        let fx: Value = serde_json::from_str(TX1).unwrap();
        let st = &fx["struct"];
        let ua = fx["args"].as_object().unwrap();

        let outpoints: Vec<Vec<u8>> = st["inputs"].as_array().unwrap().iter().map(|i| dehex(&i["outpoint"])).collect();
        let sequences: Vec<Vec<u8>> = st["inputs"].as_array().unwrap().iter().map(|i| dehex(&i["sequence"])).collect();
        let outs: Vec<(Vec<u8>, Vec<u8>)> = st["outputs"].as_array().unwrap().iter().map(|o| (dehex(&o["value"]), dehex(&o["script"]))).collect();
        let bolt_idx = st["boltInputIndex"].as_u64().unwrap() as usize;
        let header = ctx_header(&dehex(&st["version"]), &hash_prevouts(&outpoints), &hash_sequence(&sequences), &outpoints[bolt_idx]);
        let footer = ctx_footer(&dehex(&st["spentValue"]), &sequences[bolt_idx], &hash_outputs(&outs), &dehex(&st["locktime"]), &dehex(&st["sighashType"]));

        let args = simple_spend_unlock_args(
            &dehex(&ua["pubKey"]), &dehex(&st["spentLockScript"]), &outpoints[1 - bolt_idx],
            &dehex(&ua["changeOutput"]), &dehex(&ua["beneficiaryPubKeyHash"]), &header, &footer, &dehex(&ua["sig"]),
        );
        let built = fill_unlocking_script(&min_simple_discount_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "Discount transfer unlock");
    }

    /// B-5 melt: reuses melt_unlock_args + the Discount artifact.
    #[test]
    fn melt_unlock_matches_fixture() {
        const TX5: &str = include_str!("../../tests/fixtures/discount_spend_tx5.json");
        let fx: Value = serde_json::from_str(TX5).unwrap();
        let ua = fx["args"].as_object().unwrap();
        let args = melt_unlock_args(&dehex(&ua["pubKey"]), &dehex(&ua["sig"]));
        let built = fill_unlocking_script(&min_simple_discount_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "Discount melt unlock");
    }
}
