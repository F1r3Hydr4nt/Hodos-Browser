//! sx template filler: turn a compiled contract artifact + arg values into
//! script bytes, byte-identical to the sx simulator (`processRecombinants` in
//! sx/src/simulator.js) and @bsv/sdk's push encoding.
//!
//! Fill rule per recombinant entry:
//!   - string  -> raw literal script bytes (appended verbatim)
//!   - number  -> push the value of lockArgs[idx] (minimal push, matching writeBin)
//!   - object{repeat,...} -> dynamic repeat block (not needed by MinSimpleBolt; TODO)

use std::collections::HashMap;
use serde::Deserialize;
use serde_json::Value;

/// A compiled contract artifact (subset needed for script filling). Extra
/// fields in the JSON (lockOps, unlockOps, unlock side) are ignored.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Artifact {
    pub filename: String,
    pub lock_args: Vec<String>,
    pub locking_recombinants: Vec<Value>,
}

impl Artifact {
    pub fn from_json(s: &str) -> Result<Artifact, String> {
        serde_json::from_str(s).map_err(|e| format!("artifact parse error: {e}"))
    }
}

/// The frozen MinSimpleBolt artifact (copied from sx/bolt/production/artifacts).
pub const MIN_SIMPLE_BOLT_JSON: &str = include_str!("artifacts/MinSimpleBolt.json");

pub fn min_simple_bolt() -> Artifact {
    Artifact::from_json(MIN_SIMPLE_BOLT_JSON).expect("embedded MinSimpleBolt artifact is valid")
}

/// Append a minimal-encoded data push (matches @bsv/sdk writeBin).
pub fn push_data(out: &mut Vec<u8>, data: &[u8]) {
    let n = data.len();
    if n == 0 {
        out.push(0x00); // OP_0 (empty push)
        return;
    }
    if n == 1 {
        let b = data[0];
        if (1..=16).contains(&b) {
            out.push(0x50 + b); // OP_1 ..= OP_16
            return;
        }
        if b == 0x81 {
            out.push(0x4f); // OP_1NEGATE
            return;
        }
        // otherwise a normal 1-byte direct push
    }
    if n <= 75 {
        out.push(n as u8);
    } else if n <= 0xff {
        out.push(0x4c); // OP_PUSHDATA1
        out.push(n as u8);
    } else if n <= 0xffff {
        out.push(0x4d); // OP_PUSHDATA2
        out.extend_from_slice(&(n as u16).to_le_bytes());
    } else {
        out.push(0x4e); // OP_PUSHDATA4
        out.extend_from_slice(&(n as u32).to_le_bytes());
    }
    out.extend_from_slice(data);
}

/// Fill an artifact's locking recombinants with the given arg values (by name).
pub fn fill_locking_script(
    art: &Artifact,
    args: &HashMap<String, Vec<u8>>,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    for part in &art.locking_recombinants {
        match part {
            Value::String(hex_lit) => {
                let bytes = hex::decode(hex_lit)
                    .map_err(|e| format!("literal recombinant hex decode: {e}"))?;
                out.extend_from_slice(&bytes);
            }
            Value::Number(num) => {
                let idx = num
                    .as_u64()
                    .ok_or_else(|| format!("recombinant arg index not a uint: {num}"))?
                    as usize;
                let name = art
                    .lock_args
                    .get(idx)
                    .ok_or_else(|| format!("recombinant arg index {idx} out of range"))?;
                let val = args
                    .get(name)
                    .ok_or_else(|| format!("missing value for lockArg '{name}'"))?;
                push_data(&mut out, val);
            }
            other => {
                return Err(format!(
                    "unsupported recombinant (dynamic repeat blocks not yet ported): {other}"
                ))
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_data_encodings() {
        let mut e = Vec::new();
        push_data(&mut e, &[]);
        assert_eq!(e, vec![0x00]); // empty -> OP_0

        let mut one = Vec::new();
        push_data(&mut one, &[0x20]);
        assert_eq!(one, vec![0x01, 0x20]); // direct 1-byte

        let mut small = Vec::new();
        push_data(&mut small, &[0x05]);
        assert_eq!(small, vec![0x55]); // OP_5

        let mut p20 = Vec::new();
        push_data(&mut p20, &[0xaa; 20]);
        assert_eq!(p20[0], 0x14);
        assert_eq!(p20.len(), 21);

        let mut p200 = Vec::new();
        push_data(&mut p200, &[0xbb; 200]);
        assert_eq!(&p200[0..2], &[0x4c, 200]); // PUSHDATA1
    }

    #[test]
    fn artifact_loads() {
        let art = min_simple_bolt();
        assert_eq!(art.filename, "MinSimpleBolt.sx");
        assert_eq!(art.lock_args.len(), 6);
        assert_eq!(art.locking_recombinants.len(), 7);
    }

    /// B-0 golden: the Rust filler reproduces the @bsv/sdk-built lock script
    /// byte-for-byte for the same arg values (fixture from sx/scripts/b0_golden.cjs).
    #[test]
    fn fill_matches_golden_min_simple_bolt_lock() {
        const GOLDEN: &str = include_str!("../../tests/fixtures/minsimplebolt_lock.json");
        let g: Value = serde_json::from_str(GOLDEN).unwrap();
        let expected = g["lockHex"].as_str().unwrap();

        let mut args: HashMap<String, Vec<u8>> = HashMap::new();
        for (k, v) in g["args"].as_object().unwrap() {
            let hexs = v.as_str().unwrap();
            let bytes = if hexs.is_empty() { Vec::new() } else { hex::decode(hexs).unwrap() };
            args.insert(k.clone(), bytes);
        }

        let art = min_simple_bolt();
        let built = fill_locking_script(&art, &args).expect("fill");
        assert_eq!(hex::encode(&built), expected, "MinSimpleBolt lock script byte mismatch");
    }
}
