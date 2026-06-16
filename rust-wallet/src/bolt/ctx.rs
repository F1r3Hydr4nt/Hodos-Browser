//! BIP143 sighash-preimage (CTX) builder for BOLT spends (Layer B, B-2-unlock-TX).
//!
//! The unlock's ctxHeader/ctxFooter args are slices of the BIP143 preimage:
//!   preimage = version(4) ++ hashPrevouts(32) ++ hashSequence(32) ++ outpoint(36)
//!              ++ varint(scriptCodeLen) ++ scriptCode
//!              ++ value(8) ++ sequence(4) ++ hashOutputs(32) ++ nLocktime(4) ++ sighashType(4)
//!   ctxHeader = preimage[0..104]   (the four pre-scriptCode fields)
//!   ctxFooter = preimage[..52 tail] (the five post-scriptCode fields)
//! BOLT uses sighashType 0x41 (SIGHASH_ALL | FORKID).

use sha2::{Digest, Sha256};

use super::lib::varint;

/// Double SHA-256.
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    let first = Sha256::digest(data);
    let second = Sha256::digest(first);
    let mut out = [0u8; 32];
    out.copy_from_slice(&second);
    out
}

/// hashPrevouts = sha256d(concat of each input's 36-byte outpoint).
pub fn hash_prevouts(outpoints: &[Vec<u8>]) -> [u8; 32] {
    let mut buf = Vec::new();
    for o in outpoints {
        buf.extend_from_slice(o);
    }
    sha256d(&buf)
}

/// hashSequence = sha256d(concat of each input's 4-byte sequence).
pub fn hash_sequence(sequences: &[Vec<u8>]) -> [u8; 32] {
    let mut buf = Vec::new();
    for s in sequences {
        buf.extend_from_slice(s);
    }
    sha256d(&buf)
}

/// hashOutputs = sha256d(concat of each output's value(8) ++ varint(scriptLen) ++ script).
pub fn hash_outputs(outputs: &[(Vec<u8>, Vec<u8>)]) -> [u8; 32] {
    let mut buf = Vec::new();
    for (value, script) in outputs {
        buf.extend_from_slice(value);
        buf.extend_from_slice(&varint(script.len()));
        buf.extend_from_slice(script);
    }
    sha256d(&buf)
}

/// ctxHeader = version ++ hashPrevouts ++ hashSequence ++ outpoint (104 bytes).
pub fn ctx_header(version: &[u8], hash_prevouts: &[u8; 32], hash_sequence: &[u8; 32], outpoint: &[u8]) -> Vec<u8> {
    let mut h = Vec::with_capacity(104);
    h.extend_from_slice(version);
    h.extend_from_slice(hash_prevouts);
    h.extend_from_slice(hash_sequence);
    h.extend_from_slice(outpoint);
    h
}

/// ctxFooter = value ++ sequence ++ hashOutputs ++ nLocktime ++ sighashType (52 bytes).
pub fn ctx_footer(
    value: &[u8],
    sequence: &[u8],
    hash_outputs: &[u8; 32],
    locktime: &[u8],
    sighash_type: &[u8],
) -> Vec<u8> {
    let mut f = Vec::with_capacity(52);
    f.extend_from_slice(value);
    f.extend_from_slice(sequence);
    f.extend_from_slice(hash_outputs);
    f.extend_from_slice(locktime);
    f.extend_from_slice(sighash_type);
    f
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use super::*;

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

    /// B-2-unlock-TX: the BIP143 ctxHeader/ctxFooter computed from tx1's wire
    /// structure match the decoded unlock fixture byte-for-byte.
    #[test]
    fn ctx_header_footer_match_tx1_fixture() {
        const STRUCT: &str = include_str!("../../tests/fixtures/minsimplebolt_tx1_struct.json");
        const UNLOCK: &str = include_str!("../../tests/fixtures/minsimplebolt_unlock_tx1.json");
        let st: Value = serde_json::from_str(STRUCT).unwrap();
        let un: Value = serde_json::from_str(UNLOCK).unwrap();

        let inputs = st["inputs"].as_array().unwrap();
        let outpoints: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["outpoint"])).collect();
        let sequences: Vec<Vec<u8>> = inputs.iter().map(|i| dehex(&i["sequence"])).collect();
        let outputs: Vec<(Vec<u8>, Vec<u8>)> = st["outputs"]
            .as_array().unwrap().iter()
            .map(|o| (dehex(&o["value"]), dehex(&o["script"])))
            .collect();

        let bolt_idx = st["boltInputIndex"].as_u64().unwrap() as usize;
        let hp = hash_prevouts(&outpoints);
        let hs = hash_sequence(&sequences);
        let ho = hash_outputs(&outputs);

        let header = ctx_header(&dehex(&st["version"]), &hp, &hs, &outpoints[bolt_idx]);
        let footer = ctx_footer(
            &dehex(&st["spentValue"]),
            &sequences[bolt_idx],
            &ho,
            &dehex(&st["locktime"]),
            &dehex(&st["sighashType"]),
        );

        assert_eq!(hex::encode(&header), un["args"]["ctxHeader"].as_str().unwrap(), "ctxHeader");
        assert_eq!(hex::encode(&footer), un["args"]["ctxFooter"].as_str().unwrap(), "ctxFooter");
        assert_eq!(header.len(), 104);
        assert_eq!(footer.len(), 52);
    }
}
