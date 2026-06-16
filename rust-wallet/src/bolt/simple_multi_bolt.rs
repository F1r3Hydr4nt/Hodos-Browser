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

use super::lib::varint;
use super::sx_template::simple_multi_bolt;

/// SMB unlock scriptCode suffix (OP_CHECKSIGVERIFY OP_ENDIF), as for MinSimpleBolt.
const UNLOCK_SCRIPT_CODE: [u8; 2] = [0xad, 0x68];

/// Ancestor-less SMB transfer/settle unlock args (the common case: all 185
/// ancestor*/other args empty; 13 populated). `pub_key_hash1` is the new owner,
/// `next_txo_type` the produced output's type (e.g. 0x21), `input_index_n` the
/// input index byte. ctx_header/footer come from bolt::ctx over the spend tx;
/// fund_outpoint/change_output/sig are wallet-chosen. Split/merge (ancestor-
/// populated) spends use getAncestorPieceFungible (follow-on).
#[allow(clippy::too_many_arguments)]
pub fn transfer_unlock_args(
    owner_pubkey: &[u8],
    spent_lock: &[u8],
    fund_outpoint: &[u8],
    change_output: &[u8],
    pub_key_hash1: &[u8],
    next_txo_type: &[u8],
    input_index_n: &[u8],
    ctx_header: &[u8],
    ctx_footer: &[u8],
    sig: &[u8],
) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    for name in &simple_multi_bolt().unlock_args {
        m.insert(name.clone(), Vec::new());
    }
    m.insert("pubKey".into(), owner_pubkey.to_vec());
    m.insert("ctxCodeLockScriptCode".into(), spent_lock.to_vec());
    m.insert("ctxCodeLockLen".into(), varint(spent_lock.len()));
    m.insert("ctxCodeUnlockScriptCode".into(), UNLOCK_SCRIPT_CODE.to_vec());
    m.insert("ctxCodeLen".into(), varint(spent_lock.len() + UNLOCK_SCRIPT_CODE.len()));
    m.insert("fundOutpoint".into(), fund_outpoint.to_vec());
    m.insert("changeOutput".into(), change_output.to_vec());
    m.insert("pubKeyHash1".into(), pub_key_hash1.to_vec());
    m.insert("nextTxoType".into(), next_txo_type.to_vec());
    m.insert("inputIndexN".into(), input_index_n.to_vec());
    m.insert("ctxHeader".into(), ctx_header.to_vec());
    m.insert("ctxFooter".into(), ctx_footer.to_vec());
    m.insert("sig".into(), sig.to_vec());
    m
}

/// SMB split unlock args (e.g. tx7): a single bolt input produces TWO owner
/// outputs, so both `pub_key_hash1` and `pub_key_hash2` are set, plus the
/// 16-byte `next_balance_commit` carrying the remainder balance, and
/// `next_txo_type` is the split type (0x23). Otherwise identical to a transfer.
#[allow(clippy::too_many_arguments)]
pub fn split_unlock_args(
    owner_pubkey: &[u8],
    spent_lock: &[u8],
    fund_outpoint: &[u8],
    change_output: &[u8],
    pub_key_hash1: &[u8],
    pub_key_hash2: &[u8],
    next_balance_commit: &[u8],
    next_txo_type: &[u8],
    input_index_n: &[u8],
    ctx_header: &[u8],
    ctx_footer: &[u8],
    sig: &[u8],
) -> HashMap<String, Vec<u8>> {
    let mut m = transfer_unlock_args(
        owner_pubkey, spent_lock, fund_outpoint, change_output,
        pub_key_hash1, next_txo_type, input_index_n, ctx_header, ctx_footer, sig,
    );
    m.insert("pubKeyHash2".into(), pub_key_hash2.to_vec());
    m.insert("nextBalanceCommit".into(), next_balance_commit.to_vec());
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::ctx::{ctx_footer, ctx_header, hash_outputs, hash_prevouts, hash_sequence};
    use crate::bolt::sign::{build_preimage, preimage_sighash, verify_sig};
    use crate::bolt::sx_template::{fill_locking_script, fill_unlocking_script};
    use serde_json::Value;

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

    /// Derive the BIP143 ctxHeader(104)/ctxFooter(52) for a spend fixture's bolt
    /// input from its decoded `struct` (version/locktime/sighashType + per-input
    /// outpoints/sequences + outputs + spentValue), via bolt::ctx.
    fn ctx_from_struct(st: &Value) -> (Vec<u8>, Vec<u8>) {
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
        (header, footer)
    }

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

    /// B-4 transfer: the ancestor-less SMB transfer unlock reproduces tx1's
    /// bolt-input unlock (6217 B, 13/198 args populated) byte-for-byte, with
    /// ctxHeader/ctxFooter derived via bolt::ctx; also cross-checks the fixture
    /// sig verifies against the assembled preimage (scriptCode = lock).
    #[test]
    fn transfer_unlock_matches_fixtures() {
        const TX1: &str = include_str!("../../tests/fixtures/smb_spend_tx1.json");
        const TX3: &str = include_str!("../../tests/fixtures/smb_spend_tx3.json");
        for (label, raw) in [("tx1", TX1), ("tx3", TX3)] {
            let fx: Value = serde_json::from_str(raw).unwrap();
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

            // sig cross-check: scriptCode = the spent lock only (post-codeseparator)
            let preimage = build_preimage(&header, &spent_lock, &footer);
            assert!(
                verify_sig(&dehex(&ua["sig"]), &preimage_sighash(&preimage), &owner).unwrap(),
                "{label} sig verifies against assembled preimage"
            );

            let fund_outpoint = outpoints[1 - bolt_idx].clone();
            let args = transfer_unlock_args(
                &owner, &spent_lock, &fund_outpoint, &dehex(&ua["changeOutput"]),
                &dehex(&ua["pubKeyHash1"]), &dehex(&ua["nextTxoType"]), &dehex(&ua["inputIndexN"]),
                &header, &footer, &dehex(&ua["sig"]),
            );
            let built = fill_unlocking_script(&simple_multi_bolt(), &args).expect("fill");
            assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "{label} SMB transfer unlock mismatch");
        }
    }

    /// B-4 single-input variants: tx2/tx4 are settles (empty pubKeyHash1, no
    /// balanceCommit, nextTxoType=0x20) reproduced by transfer_unlock_args with an
    /// empty new-owner; tx7 is a split (pubKeyHash1+pubKeyHash2+nextBalanceCommit,
    /// nextTxoType=0x23) via split_unlock_args. All ancestor-less, single bolt input.
    #[test]
    fn settle_and_split_unlock_match_fixtures() {
        // settles: tx2, tx4 (transfer builder, empty pubKeyHash1)
        for label in ["tx2", "tx4"] {
            let raw = match label {
                "tx2" => include_str!("../../tests/fixtures/smb_spend_tx2.json"),
                _ => include_str!("../../tests/fixtures/smb_spend_tx4.json"),
            };
            let fx: Value = serde_json::from_str(raw).unwrap();
            let (header, footer) = ctx_from_struct(&fx["struct"]);
            let ua = fx["args"].as_object().unwrap();
            let args = transfer_unlock_args(
                &dehex(&ua["pubKey"]), &dehex(&fx["struct"]["spentLockScript"]),
                &dehex(&ua["fundOutpoint"]), &dehex(&ua["changeOutput"]),
                &dehex(&ua["pubKeyHash1"]), &dehex(&ua["nextTxoType"]), &dehex(&ua["inputIndexN"]),
                &header, &footer, &dehex(&ua["sig"]),
            );
            let built = fill_unlocking_script(&simple_multi_bolt(), &args).expect("fill");
            assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "{label} SMB settle unlock mismatch");
        }

        // split: tx7
        let fx: Value = serde_json::from_str(include_str!("../../tests/fixtures/smb_spend_tx7.json")).unwrap();
        let (header, footer) = ctx_from_struct(&fx["struct"]);
        let ua = fx["args"].as_object().unwrap();
        let args = split_unlock_args(
            &dehex(&ua["pubKey"]), &dehex(&fx["struct"]["spentLockScript"]),
            &dehex(&ua["fundOutpoint"]), &dehex(&ua["changeOutput"]),
            &dehex(&ua["pubKeyHash1"]), &dehex(&ua["pubKeyHash2"]), &dehex(&ua["nextBalanceCommit"]),
            &dehex(&ua["nextTxoType"]), &dehex(&ua["inputIndexN"]),
            &header, &footer, &dehex(&ua["sig"]),
        );
        let built = fill_unlocking_script(&simple_multi_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "tx7 SMB split unlock mismatch");
    }

    /// B-4 merge (2-bolt-input swap/merge, tx5): the engine fills the full 198-arg
    /// unlocking layout for BOTH bolt inputs byte-for-byte. Each input is ancestor-
    /// less (185 ancestor* args empty) with merge-specific interop*/nextBalanceCommit/
    /// inputIndexN populated. This proves the filler assembles the 2-input merge
    /// layout given the decoded args (deriving interop*/nextBalanceCommit from the
    /// two input tokens is the remaining fungible-merge wallet work).
    #[test]
    fn merge_unlock_matches_fixture_tx5() {
        const MERGE: &str = include_str!("../../tests/fixtures/smb_merge_tx5.json");
        let fx: Value = serde_json::from_str(MERGE).unwrap();
        let art = simple_multi_bolt();
        let bolt_inputs = fx["boltInputs"].as_array().unwrap();
        assert_eq!(bolt_inputs.len(), 2, "tx5 has two bolt inputs (merge)");

        for bi in bolt_inputs {
            let vin = bi["vin"].as_u64().unwrap();
            let ua = bi["args"].as_object().unwrap();
            let mut args: HashMap<String, Vec<u8>> = HashMap::new();
            for name in &art.unlock_args {
                let v = ua.get(name).map(dehex).unwrap_or_default();
                args.insert(name.clone(), v);
            }
            let built = fill_unlocking_script(&art, &args).expect("fill");
            assert_eq!(
                hex::encode(&built),
                bi["unlockHex"].as_str().unwrap(),
                "SMB merge unlock mismatch for bolt input vin {vin}"
            );
        }
    }

    /// B-4 ancestor-populated fungible spends (tx6, tx8 - the getAncestorPieceFungible
    /// shapes). tx6 carries TWO ancestor txs (A+B, each single-bolt-input, 82 ancestor
    /// args); tx8 carries ONE ancestor tx with TWO bolt inputs (70 ancestor args). This
    /// proves the filler assembles the full ancestor-POPULATED 198-arg layout (8.0-8.3kB
    /// unlocks) byte-for-byte from decoded args. DERIVING the ancestor* args by
    /// reconstructing the ancestor txs from chain data (boltLib getAncestorPieceFungible)
    /// is the remaining heavy fungible-ancestor wallet work; the engine layer is now
    /// proven across every SMB spend shape.
    #[test]
    fn ancestor_fungible_unlock_matches_fixtures() {
        let art = simple_multi_bolt();
        for (label, raw) in [
            ("tx6", include_str!("../../tests/fixtures/smb_full_tx6.json")),
            ("tx8", include_str!("../../tests/fixtures/smb_full_tx8.json")),
        ] {
            let fx: Value = serde_json::from_str(raw).unwrap();
            for bi in fx["boltInputs"].as_array().unwrap() {
                let vin = bi["vin"].as_u64().unwrap();
                let ua = bi["args"].as_object().unwrap();
                let mut args: HashMap<String, Vec<u8>> = HashMap::new();
                for name in &art.unlock_args {
                    args.insert(name.clone(), ua.get(name).map(dehex).unwrap_or_default());
                }
                let built = fill_unlocking_script(&art, &args).expect("fill");
                assert_eq!(
                    hex::encode(&built),
                    bi["unlockHex"].as_str().unwrap(),
                    "{label} SMB ancestor-fungible unlock mismatch (vin {vin})"
                );
            }
        }
    }
}
