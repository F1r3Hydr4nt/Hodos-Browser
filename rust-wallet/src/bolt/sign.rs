//! BOLT spend signing (Layer B / B-2 live sign).
//!
//! The unlock `sig` arg is an ECDSA signature over sha256d(BIP143 preimage),
//! with the sighash-type byte (0x41 = ALL|FORKID) appended. The preimage is the
//! standard BIP143 form:
//!   ctxHeader(104) ++ varint(scriptCodeLen) ++ scriptCode ++ ctxFooter(52)
//! where scriptCode = the LOCKING script being spent (the bolt lock, 1265 B for
//! MinSimpleBolt - empirically the sig commits to the lock only, NOT lock+ad68;
//! the ad68 / ctxCodeLen is the contract's internal reconstruction, not the sig).

use secp256k1::{ecdsa::Signature, Message, PublicKey, Secp256k1, SecretKey};

use super::ctx::sha256d;
use super::lib::varint;

/// BSV sighash type for BOLT spends: SIGHASH_ALL | FORKID.
pub const SIGHASH_ALL_FORKID: u8 = 0x41;

/// Assemble the BIP143 sighash preimage. `script_code` is the locking script of
/// the output being spent (the bolt lock). The post-OP_CODESEPARATOR checksig
/// suffix (ad68) is NOT part of the scriptCode the sig commits to - the
/// interpreter strips everything up to the last code separator.
pub fn build_preimage(ctx_header: &[u8], script_code: &[u8], ctx_footer: &[u8]) -> Vec<u8> {
    let code_len = varint(script_code.len());
    let mut p =
        Vec::with_capacity(ctx_header.len() + code_len.len() + script_code.len() + ctx_footer.len());
    p.extend_from_slice(ctx_header);
    p.extend_from_slice(&code_len);
    p.extend_from_slice(script_code);
    p.extend_from_slice(ctx_footer);
    p
}

/// The 32-byte sighash for a BOLT preimage (double-SHA256).
pub fn preimage_sighash(preimage: &[u8]) -> [u8; 32] {
    sha256d(preimage)
}

/// Sign a 32-byte sighash with `priv_key` (32 bytes); returns DER signature ++
/// sighash-type byte, as the unlock `sig` arg expects.
pub fn sign_sighash(sighash: &[u8; 32], priv_key: &[u8]) -> Result<Vec<u8>, String> {
    let secp = Secp256k1::signing_only();
    let sk = SecretKey::from_slice(priv_key).map_err(|e| format!("bad privkey: {e}"))?;
    let msg = Message::from_digest_slice(sighash).map_err(|e| format!("bad sighash: {e}"))?;
    let sig = secp.sign_ecdsa(&msg, &sk);
    let mut der = sig.serialize_der().to_vec();
    der.push(SIGHASH_ALL_FORKID);
    Ok(der)
}

/// Verify an unlock `sig` (DER ++ sighash-type byte) over a 32-byte sighash for
/// a 33-byte compressed pubkey. Tolerates the trailing sighash-type byte.
pub fn verify_sig(sig_with_type: &[u8], sighash: &[u8; 32], pub_key: &[u8]) -> Result<bool, String> {
    if sig_with_type.is_empty() {
        return Err("empty sig".into());
    }
    let der = &sig_with_type[..sig_with_type.len() - 1]; // strip sighash-type byte
    let secp = Secp256k1::verification_only();
    let sig = Signature::from_der(der).map_err(|e| format!("bad der sig: {e}"))?;
    let msg = Message::from_digest_slice(sighash).map_err(|e| format!("bad sighash: {e}"))?;
    let pk = PublicKey::from_slice(pub_key).map_err(|e| format!("bad pubkey: {e}"))?;
    Ok(secp.verify_ecdsa(&msg, &sig, &pk).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn dehex(v: &Value) -> Vec<u8> {
        let s = v.as_str().unwrap();
        if s.is_empty() { Vec::new() } else { hex::decode(s).unwrap() }
    }

    /// Validation: assemble tx1's BIP143 preimage from the fixture ctx pieces and
    /// confirm the fixture's real signature verifies against it for the owner
    /// pubkey. Proves the preimage assembly + sig verification are correct
    /// (no privkey needed - uses the signature the sx simulator produced).
    #[test]
    fn fixture_sig_verifies_against_assembled_preimage() {
        const TX1: &str = include_str!("../../tests/fixtures/minsimplebolt_unlock_tx1.json");
        let fx: Value = serde_json::from_str(TX1).unwrap();
        let a = &fx["args"];

        // scriptCode = the locking script only (post-codeseparator excludes ad68)
        let preimage = build_preimage(
            &dehex(&a["ctxHeader"]),
            &dehex(&a["ctxCodeLockScriptCode"]),
            &dehex(&a["ctxFooter"]),
        );
        let sighash = preimage_sighash(&preimage);
        let sig = dehex(&a["sig"]);
        let pubkey = dehex(&a["pubKey"]);

        assert!(
            verify_sig(&sig, &sighash, &pubkey).unwrap(),
            "fixture sig should verify against the assembled preimage"
        );
    }

    /// Live signing round-trip: sign a sighash with a key and verify with its pubkey.
    #[test]
    fn sign_then_verify_roundtrip() {
        let secp = Secp256k1::new();
        let sk_bytes = [0x11u8; 32];
        let sk = SecretKey::from_slice(&sk_bytes).unwrap();
        let pk = PublicKey::from_secret_key(&secp, &sk).serialize().to_vec();

        let sighash = [0x42u8; 32];
        let sig = sign_sighash(&sighash, &sk_bytes).unwrap();
        assert_eq!(*sig.last().unwrap(), SIGHASH_ALL_FORKID);
        assert!(verify_sig(&sig, &sighash, &pk).unwrap());

        // wrong message must not verify
        let other = [0x43u8; 32];
        assert!(!verify_sig(&sig, &other, &pk).unwrap());
    }
}
