//! BEEF production for a BOLT mint (B1/B2 slice, toward Layer E proof export).
//!
//! Wraps a freshly-built mint tx + its funding parent into a BRC-62 BEEF bundle
//! so a verifier can walk ancestry from the mint down to a proven leaf. The mint
//! is unproven at build time (unmined), so this produces the parent-chain BEEF;
//! a merkle proof (BUMP) is grafted in later via `beef.add_tsc_merkle_proof`
//! once the broadcast tx is mined (E-2's proof export).
//!
//! This is a pure assembly over `crate::beef` - no network, no AppState - so it
//! is unit-testable offline and reuses the wallet's audited BEEF serializer.

use crate::beef::Beef;

/// Build a V2 BEEF whose main tx is the mint and which carries the given parent
/// raw txs (e.g. the funding tx the mint spends) for ancestry. Returns BEEF hex.
pub fn build_mint_beef(mint_raw_hex: &str, parent_raw_hexes: &[&str]) -> Result<String, String> {
    let mut beef = Beef::new();
    for p in parent_raw_hexes {
        let bytes = hex::decode(p).map_err(|e| format!("parent hex decode: {e}"))?;
        beef.add_parent_transaction(bytes);
    }
    let main = hex::decode(mint_raw_hex).map_err(|e| format!("mint hex decode: {e}"))?;
    beef.set_main_transaction(main);
    beef.sort_topologically();
    beef.to_hex()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::lib::hash160;
    use crate::bolt::mint::{build_msbbolt_mint, Funding};
    use crate::transaction::{OutPoint, Script, Transaction, TxInput, TxOutput};
    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    /// Build a real funding tx paying `value` to `pubkey`'s P2PKH at vout 0, so
    /// the mint can spend a parent that actually exists in the bundle (real txid).
    fn build_funding_tx(pubkey: &[u8], value: i64) -> (String, String) {
        let p2pkh = Script::p2pkh_locking_script(&hash160(pubkey)).unwrap().bytes;
        let mut tx = Transaction::new();
        tx.version = 2;
        let mut input = TxInput::new(OutPoint::new("cc".repeat(32), 0));
        input.sequence = 0xffff_ffff;
        tx.add_input(input);
        tx.add_output(TxOutput::new(value, p2pkh));
        (tx.to_hex().unwrap(), tx.txid().unwrap())
    }

    /// B1/B2 BEEF production: the mint + its funding parent round-trip through the
    /// wallet's BEEF serializer with the mint as main tx, the funding tx present
    /// as a parent, and the mint->funding ancestry link resolved in the bundle.
    #[test]
    fn mint_beef_round_trips_with_funding_parent() {
        let secp = Secp256k1::new();
        let sk = [0x2au8; 32];
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap())
            .serialize()
            .to_vec();
        let balance = hex::decode("0000000000000000000000000000cafe").unwrap();

        // real funding parent (its txid is derived from its bytes)
        let (funding_raw, funding_txid) = build_funding_tx(&pk, 100_000);
        let funding = Funding {
            txid: funding_txid.clone(),
            vout: 0,
            value: 100_000,
            priv_key: sk,
            pub_key: pk.clone(),
        };
        let mint = build_msbbolt_mint(&pk, &balance, &funding, 200).unwrap();

        let beef_hex = build_mint_beef(&mint.raw_tx_hex, &[&funding_raw]).unwrap();

        // round-trips through the audited parser
        let parsed = Beef::from_hex(&beef_hex).expect("BEEF parses");
        assert_eq!(
            hex::encode(parsed.main_transaction().expect("has main")),
            mint.raw_tx_hex,
            "BEEF main tx is the mint"
        );
        assert!(
            parsed.parent_transactions().iter().any(|p| hex::encode(p) == funding_raw),
            "funding tx present as a BEEF parent"
        );
        // the mint is locatable by txid; extract returns the mint raw tx
        assert!(parsed.find_txid(&mint.txid).is_some(), "mint txid indexed in BEEF");
        assert_eq!(
            Beef::extract_raw_tx_hex(&beef_hex).unwrap(),
            mint.raw_tx_hex,
            "extract_raw_tx_hex yields the mint"
        );
    }

    /// Emit the mint BEEF as a committed fixture (deterministic) for a cross-repo
    /// @bsv `Transaction.fromBEEF` byte-format check (priv-chain side).
    #[test]
    fn emit_mint_beef_fixture() {
        let secp = Secp256k1::new();
        let sk = [0x2au8; 32];
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap())
            .serialize()
            .to_vec();
        let balance = hex::decode("0000000000000000000000000000cafe").unwrap();
        let (funding_raw, funding_txid) = build_funding_tx(&pk, 100_000);
        let funding = Funding {
            txid: funding_txid, vout: 0, value: 100_000, priv_key: sk, pub_key: pk.clone(),
        };
        let mint = build_msbbolt_mint(&pk, &balance, &funding, 200).unwrap();
        let beef_hex = build_mint_beef(&mint.raw_tx_hex, &[&funding_raw]).unwrap();

        let json = format!(
            "{{\n  \"beefHex\": \"{}\",\n  \"mintTxid\": \"{}\",\n  \"mintRawTxHex\": \"{}\",\n  \"fundingRawTxHex\": \"{}\"\n}}\n",
            beef_hex, mint.txid, mint.raw_tx_hex, funding_raw
        );
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(format!("{dir}/b1_mint_beef.json"), json).unwrap();
    }
}
