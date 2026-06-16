//! boltLib helpers - Rust port of ts-bolt/src/boltLib.ts (Layer B).
//!
//! Byte-level utilities shared by the contract builders: hash160, outpoint
//! construction, etc. Grown as the contract ports (B-2..) test-drive them.

use ripemd::Ripemd160;
use sha2::{Digest, Sha256};

/// HASH160 = RIPEMD160(SHA256(data)). Used to derive a pubKeyHash from a pubkey.
pub fn hash160(data: &[u8]) -> [u8; 20] {
    let sha = Sha256::digest(data);
    let rip = Ripemd160::digest(sha);
    let mut out = [0u8; 20];
    out.copy_from_slice(&rip);
    out
}

/// Bitcoin CompactSize (varint) encoding of a length.
pub fn varint(n: usize) -> Vec<u8> {
    if n < 0xfd {
        vec![n as u8]
    } else if n <= 0xffff {
        let mut v = vec![0xfd];
        v.extend_from_slice(&(n as u16).to_le_bytes());
        v
    } else if n <= 0xffff_ffff {
        let mut v = vec![0xfe];
        v.extend_from_slice(&(n as u32).to_le_bytes());
        v
    } else {
        let mut v = vec![0xff];
        v.extend_from_slice(&(n as u64).to_le_bytes());
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash160_of_msbolt_owner_pubkey() {
        // owner/issuer pubkey from the MSBolt sim mint (deterministic bip32 key)
        let pk = hex::decode("035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7").unwrap();
        assert_eq!(hex::encode(hash160(&pk)), "7be85b07272c9abe689fed4f92de46b5dd0a18c7");
    }

    #[test]
    fn varint_encoding() {
        assert_eq!(varint(0), vec![0x00]);
        assert_eq!(varint(252), vec![0xfc]);
        assert_eq!(varint(253), vec![0xfd, 0xfd, 0x00]);
        assert_eq!(varint(1265), vec![0xfd, 0xf1, 0x04]); // ctxCodeLockLen
        assert_eq!(varint(1267), vec![0xfd, 0xf3, 0x04]); // ctxCodeLen
    }
}
