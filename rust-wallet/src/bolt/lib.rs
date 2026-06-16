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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash160_of_msbolt_owner_pubkey() {
        // owner/issuer pubkey from the MSBolt sim mint (deterministic bip32 key)
        let pk = hex::decode("035f9b0b33eb636964205e77e71b5243552c2e4665be1e7ef1b6925211fd1bc0f7").unwrap();
        assert_eq!(hex::encode(hash160(&pk)), "7be85b07272c9abe689fed4f92de46b5dd0a18c7");
    }
}
