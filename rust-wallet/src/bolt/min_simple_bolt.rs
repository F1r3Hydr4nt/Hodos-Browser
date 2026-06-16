//! MinSimpleBolt (NFT base) builder - Rust port (Layer B, B-2).
//!
//! B-2 mint-lock half: derive the genesis mint's 6 lockArgs and fill the bolt
//! output locking script, byte-matching the SxSimulator fixture (tx0 out0).
//! The transfer commit/settle UNLOCK side (ancestor reconstruction) is B-2-unlock.

use std::collections::HashMap;

use super::lib::{hash160, varint};
use super::sx_template::{fill_locking_script, min_simple_bolt};

/// MinSimpleBolt's unlock scriptCode suffix (OP_CHECKSIGVERIFY OP_ENDIF).
const UNLOCK_SCRIPT_CODE: [u8; 2] = [0xad, 0x68];

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

/// Derive the contract-static (tx-independent) unlock args for spending a
/// MinSimpleBolt output owned by `owner_pubkey`, when the spent output is a
/// GENESIS mint (all `ancestor*` args empty). The tx-dependent args
/// (ctxHeader/ctxFooter/sig/fundOutpoint/changeOutput) come from the spend-tx
/// BIP143 preimage builder (B-2-unlock-tx) and are NOT set here.
pub fn genesis_spend_static_unlock_args(owner_pubkey: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();

    // all ancestor* args are empty when spending a genesis output
    for name in &min_simple_bolt().unlock_args {
        if name.starts_with("ancestor") {
            m.insert(name.clone(), Vec::new());
        }
    }

    // the spent output's scriptCode IS this contract's locking script (the mint lock)
    let lock = fill_locking_script(&min_simple_bolt(), &mint_lock_args(owner_pubkey))
        .expect("mint lock for scriptCode");

    m.insert("pubKey".to_string(), owner_pubkey.to_vec());
    m.insert("ctxCodeLockLen".to_string(), varint(lock.len()));
    m.insert("ctxCodeLockScriptCode".to_string(), lock.clone());
    m.insert("ctxCodeUnlockScriptCode".to_string(), UNLOCK_SCRIPT_CODE.to_vec());
    m.insert(
        "ctxCodeLen".to_string(),
        varint(lock.len() + UNLOCK_SCRIPT_CODE.len()),
    );
    m
}

/// Full unlock args for spending a GENESIS MinSimpleBolt output. Combines the
/// contract-static args (derived from `owner_pubkey`) with the per-spend args
/// the wallet supplies from its constructed spend tx: the funding outpoint, the
/// change output (wire-serialized), the beneficiary pubKeyHash (send target),
/// the BIP143 ctxHeader/ctxFooter (from bolt::ctx over the spend tx), and the
/// signature over the preimage.
#[allow(clippy::too_many_arguments)]
pub fn genesis_spend_unlock_args(
    owner_pubkey: &[u8],
    fund_outpoint: &[u8],
    change_output: &[u8],
    beneficiary_pkh: &[u8],
    ctx_header: &[u8],
    ctx_footer: &[u8],
    sig: &[u8],
) -> HashMap<String, Vec<u8>> {
    let mut m = genesis_spend_static_unlock_args(owner_pubkey);
    m.insert("fundOutpoint".to_string(), fund_outpoint.to_vec());
    m.insert("changeOutput".to_string(), change_output.to_vec());
    m.insert("beneficiaryPubKeyHash".to_string(), beneficiary_pkh.to_vec());
    m.insert("ctxHeader".to_string(), ctx_header.to_vec());
    m.insert("ctxFooter".to_string(), ctx_footer.to_vec());
    m.insert("sig".to_string(), sig.to_vec());
    m
}

/// General ancestor-less spend unlock args (tx1/2/3-type: 26 ancestor* empty).
/// Unlike `genesis_spend_unlock_args`, the spent output's lock script is passed
/// in (the wallet's UTXO) rather than re-derived - because across transfers the
/// owner pubKeyHash changes while the issuer stays fixed, so the spent lock is
/// not a freshly-derived mint lock.
#[allow(clippy::too_many_arguments)]
pub fn simple_spend_unlock_args(
    owner_pubkey: &[u8],
    spent_lock_script: &[u8],
    fund_outpoint: &[u8],
    change_output: &[u8],
    beneficiary_pkh: &[u8],
    ctx_header: &[u8],
    ctx_footer: &[u8],
    sig: &[u8],
) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    for name in &min_simple_bolt().unlock_args {
        if name.starts_with("ancestor") {
            m.insert(name.clone(), Vec::new());
        }
    }
    m.insert("pubKey".to_string(), owner_pubkey.to_vec());
    m.insert("ctxCodeLockLen".to_string(), varint(spent_lock_script.len()));
    m.insert("ctxCodeLockScriptCode".to_string(), spent_lock_script.to_vec());
    m.insert("ctxCodeUnlockScriptCode".to_string(), UNLOCK_SCRIPT_CODE.to_vec());
    m.insert("ctxCodeLen".to_string(), varint(spent_lock_script.len() + UNLOCK_SCRIPT_CODE.len()));
    m.insert("fundOutpoint".to_string(), fund_outpoint.to_vec());
    m.insert("changeOutput".to_string(), change_output.to_vec());
    m.insert("beneficiaryPubKeyHash".to_string(), beneficiary_pkh.to_vec());
    m.insert("ctxHeader".to_string(), ctx_header.to_vec());
    m.insert("ctxFooter".to_string(), ctx_footer.to_vec());
    m.insert("sig".to_string(), sig.to_vec());
    m
}

/// Melt unlock args: spend the bolt to a P2PKH (destroy the token). The melt
/// branch needs only the owner's signature + pubkey; all other 35 unlock args
/// are empty.
pub fn melt_unlock_args(owner_pubkey: &[u8], sig: &[u8]) -> HashMap<String, Vec<u8>> {
    let mut m = HashMap::new();
    for name in &min_simple_bolt().unlock_args {
        m.insert(name.clone(), Vec::new());
    }
    m.insert("pubKey".to_string(), owner_pubkey.to_vec());
    m.insert("sig".to_string(), sig.to_vec());
    m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::ctx::{ctx_footer, ctx_header, hash_outputs, hash_prevouts, hash_sequence};
    use crate::bolt::sx_template::fill_unlocking_script;
    use serde_json::Value;

    fn args_map_from_fixture(args_obj: &serde_json::Map<String, Value>) -> HashMap<String, Vec<u8>> {
        let mut m = HashMap::new();
        for (k, v) in args_obj {
            let hexs = v.as_str().unwrap();
            let bytes = if hexs.is_empty() { Vec::new() } else { hex::decode(hexs).unwrap() };
            m.insert(k.clone(), bytes);
        }
        m
    }

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

    /// B-2-unlock assembly: given the 37 unlockArg values, the generalized filler
    /// reproduces the tx1 bolt-input unlock script (2078 B) byte-for-byte. This
    /// proves the unlock uses the same fill model as the lock; the remaining work
    /// is DERIVING the 37 values (ancestor reconstruction), tracked separately.
    #[test]
    fn transfer_unlock_assembly_matches_fixture() {
        const UNLOCK: &str = include_str!("../../tests/fixtures/minsimplebolt_unlock_tx1.json");
        let fx: Value = serde_json::from_str(UNLOCK).unwrap();
        let expected = fx["unlockHex"].as_str().unwrap();
        let args = args_map_from_fixture(fx["args"].as_object().unwrap());

        // independent partial-derivation checks
        assert_eq!(
            hex::encode(args.get("pubKey").unwrap()),
            "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7",
            "spend pubKey should be the owner key"
        );
        assert_eq!(args.get("beneficiaryPubKeyHash").unwrap().len(), 20);

        let built = fill_unlocking_script(&min_simple_bolt(), &args).expect("unlock fill");
        assert_eq!(hex::encode(&built), expected, "tx1 unlock assembly mismatch");
    }

    /// B-2-unlock-derive: the contract-static unlock args derived from the owner
    /// key match the fixture (all ancestor* empty for a genesis spend; pubKey,
    /// the two length varints, the unlock scriptCode suffix, and crucially
    /// ctxCodeLockScriptCode == the mint lock we build).
    #[test]
    fn genesis_spend_static_args_match_fixture() {
        const OWNER_PUBKEY: &str =
            "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";
        let owner = hex::decode(OWNER_PUBKEY).unwrap();
        let derived = genesis_spend_static_unlock_args(&owner);

        const UNLOCK: &str = include_str!("../../tests/fixtures/minsimplebolt_unlock_tx1.json");
        let fx: Value = serde_json::from_str(UNLOCK).unwrap();
        let fxargs = fx["args"].as_object().unwrap();

        for (name, val) in &derived {
            let exp = fxargs.get(name).unwrap_or_else(|| panic!("fixture missing arg {name}"));
            assert_eq!(hex::encode(val), exp.as_str().unwrap(), "static unlock arg {name} mismatch");
        }
        // 26 ancestor* (empty) + 5 contract-static derived
        assert_eq!(derived.len(), 26 + 5, "expected 31 contract-static args");
    }

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

    /// B-2 genesis-spend COMPLETE: assemble all 37 unlock args from derivation +
    /// the spend-tx structure (+ injected sig) and reproduce tx1's bolt-input
    /// unlock byte-for-byte. fundOutpoint/changeOutput/beneficiaryPubKeyHash are
    /// derived from the tx structure; ctxHeader/ctxFooter from bolt::ctx; sig is
    /// the fixture's (needs the owner privkey to produce in a live build).
    #[test]
    fn genesis_spend_full_unlock_matches_fixture() {
        const OWNER: &str = "035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7";
        let owner = hex::decode(OWNER).unwrap();

        const STRUCT: &str = include_str!("../../tests/fixtures/minsimplebolt_tx1_struct.json");
        const UNLOCK: &str = include_str!("../../tests/fixtures/minsimplebolt_unlock_tx1.json");
        let st: Value = serde_json::from_str(STRUCT).unwrap();
        let un: Value = serde_json::from_str(UNLOCK).unwrap();
        let ua = un["args"].as_object().unwrap();

        // BIP143 ctx from the spend-tx structure
        let inputs = st["inputs"].as_array().unwrap();
        let outpoints: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["outpoint"])).collect();
        let sequences: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["sequence"])).collect();
        let outputs_raw: Vec<(Vec<u8>, Vec<u8>)> = st["outputs"].as_array().unwrap().iter()
            .map(|o| (dehex(&o["value"]), dehex(&o["script"]))).collect();
        let bolt_idx = st["boltInputIndex"].as_u64().unwrap() as usize;

        let header = ctx_header(
            &dehex(&st["version"]),
            &hash_prevouts(&outpoints),
            &hash_sequence(&sequences),
            &outpoints[bolt_idx],
        );
        let footer = ctx_footer(
            &dehex(&st["spentValue"]),
            &sequences[bolt_idx],
            &hash_outputs(&outputs_raw),
            &dehex(&st["locktime"]),
            &dehex(&st["sighashType"]),
        );

        // fundOutpoint = the non-bolt (funding) input's outpoint
        let fund_outpoint = outpoints[1 - bolt_idx].clone();

        // changeOutput = the output paying back to the owner's P2PKH (wire-serialized)
        let owner_p2pkh = {
            let mut s = vec![0x76, 0xa9, 0x14];
            s.extend_from_slice(&hash160(&owner));
            s.extend_from_slice(&[0x88, 0xac]);
            s
        };
        let change_output = outputs_raw.iter()
            .find(|(_, script)| *script == owner_p2pkh)
            .map(|(value, script)| {
                let mut o = value.clone();
                o.extend_from_slice(&varint(script.len()));
                o.extend_from_slice(script);
                o
            })
            .expect("owner change output present");

        // beneficiary + sig are spend parameters (sig needs the privkey to produce)
        let beneficiary_pkh = dehex(&un["args"]["beneficiaryPubKeyHash"]);
        let sig = dehex(&un["args"]["sig"]);

        let args = genesis_spend_unlock_args(
            &owner, &fund_outpoint, &change_output, &beneficiary_pkh, &header, &footer, &sig,
        );
        assert_eq!(args.len(), 37, "all 37 unlock args present");

        // independent cross-checks of the derived (non-injected) args
        assert_eq!(hex::encode(&fund_outpoint), ua["fundOutpoint"].as_str().unwrap());
        assert_eq!(hex::encode(&change_output), ua["changeOutput"].as_str().unwrap());

        let built = fill_unlocking_script(&min_simple_bolt(), &args).expect("unlock fill");
        assert_eq!(hex::encode(&built), un["unlockHex"].as_str().unwrap(), "full genesis unlock mismatch");
    }

    /// B-2 generalization: the ancestor-less spend builder reproduces tx1, tx2 AND
    /// tx3 unlocks byte-for-byte (non-genesis simple transfers; owner pubKeyHash
    /// changes across transfers, issuer fixed). Ancestor reconstruction is isolated
    /// to the tx4-type spend (handled separately).
    #[test]
    fn simple_spends_full_unlock_match_fixtures() {
        const TX1: &str = include_str!("../../tests/fixtures/minsimplebolt_spend_tx1.json");
        const TX2: &str = include_str!("../../tests/fixtures/minsimplebolt_spend_tx2.json");
        const TX3: &str = include_str!("../../tests/fixtures/minsimplebolt_spend_tx3.json");

        for (label, raw) in [("tx1", TX1), ("tx2", TX2), ("tx3", TX3)] {
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

            // fundOutpoint is the non-bolt (funding) input - derivable from the tx
            let fund_outpoint = outpoints[1 - bolt_idx].clone();
            assert_eq!(hex::encode(&fund_outpoint), ua["fundOutpoint"].as_str().unwrap(), "{label} fundOutpoint");

            // changeOutput, beneficiary, sig are wallet-chosen spend params (the
            // wallet picks its change, the send target, and signs)
            let change_output = dehex(&ua["changeOutput"]);

            let args = simple_spend_unlock_args(
                &owner, &spent_lock, &fund_outpoint, &change_output,
                &dehex(&ua["beneficiaryPubKeyHash"]), &header, &footer, &dehex(&ua["sig"]),
            );
            let built = fill_unlocking_script(&min_simple_bolt(), &args).expect("fill");
            assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "{label} full unlock mismatch");
        }
    }

    /// tx4 is the one spend that populates the 26 ancestor* args (reconstructing
    /// the parent tx3). Confirm the filler reproduces its 37-arg unlock (incl.
    /// populated ancestors) byte-for-byte.
    #[test]
    fn tx4_ancestor_unlock_assembly_matches_fixture() {
        const TX4: &str = include_str!("../../tests/fixtures/minsimplebolt_spend_tx4.json");
        let fx: Value = serde_json::from_str(TX4).unwrap();
        let args = args_map_from_fixture(fx["args"].as_object().unwrap());
        // sanity: ancestors are populated here
        assert!(!args.get("ancestorVer").unwrap().is_empty());
        assert!(!args.get("ancestorVin1Sig").unwrap().is_empty());
        let built = fill_unlocking_script(&min_simple_bolt(), &args).expect("fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "tx4 ancestor unlock mismatch");
    }

    /// B-2 melt: the melt unlock (only sig + pubKey populated) reproduces tx5's
    /// bolt-input unlock byte-for-byte (sig injected; needs the owner key live).
    #[test]
    fn melt_unlock_matches_fixture() {
        const TX5: &str = include_str!("../../tests/fixtures/minsimplebolt_spend_tx5.json");
        let fx: Value = serde_json::from_str(TX5).unwrap();
        let ua = fx["args"].as_object().unwrap();
        let owner = dehex(&ua["pubKey"]);
        let sig = dehex(&ua["sig"]);

        let args = melt_unlock_args(&owner, &sig);
        let built = fill_unlocking_script(&min_simple_bolt(), &args).expect("melt fill");
        assert_eq!(hex::encode(&built), fx["unlockHex"].as_str().unwrap(), "melt unlock mismatch");
    }

    // NOTE (B-2 ancestor reconstruction - BLOCKED, see PROGRESS.md): the tx4
    // ancestor* args do NOT reconstruct the immediate parent tx3. Evidence:
    //   ancestorVin1Outpoint = 2f1f1995..c9:0  (a funding/genesis-era outpoint)
    //     but tx3.inputs[0].outpoint = 94bcc238..d8:0
    //   ancestorVout1TxoType = 0x21 (settle)
    // The reconstructed ancestor follows BOLT's commit/settle lineage, not the
    // direct parent. Deriving it requires reading the sx contract's ancestor
    // reconstruction + the commit/settle tx semantics. Deferred; the ancestor-less
    // spends (mint + tx1/2/3 transfers) are fully derived above.
}
