//! MSBBolt mint transaction builder (Layer B / B-2 mint tx).
//!
//! Assembles a full genesis mint: [P2PKH funding input] -> [bolt output (1 sat,
//! MSBBolt lock) + P2PKH change], signs the funding input via the validated
//! bolt::ctx/sign BIP143 path, and emits rawTxHex + txid. This is the wallet
//! capability B1 needs (mint an identity NFT to the user's key).

use crate::transaction::{OutPoint, Script, Transaction, TxInput, TxOutput};

use super::ctx::{ctx_footer, ctx_header, hash_outputs, hash_prevouts, hash_sequence};
use super::lib::hash160;
use super::msbbolt::mint_lock_args;
use super::sign::{build_preimage, preimage_sighash, sign_sighash};
use super::sx_template::{fill_locking_script, min_simple_balance_bolt, min_simple_bolt, simple_multi_bolt};

/// A P2PKH funding UTXO the wallet controls.
pub struct Funding {
    pub txid: String,
    pub vout: u32,
    pub value: i64,
    pub priv_key: [u8; 32],
    pub pub_key: Vec<u8>, // 33-byte compressed
}

pub struct MintResult {
    pub raw_tx_hex: String,
    pub txid: String,
    pub bolt_vout: u32,
}

/// BIP143 sighash for a P2PKH input of `tx` (sighash type ALL|FORKID), using the
/// validated bolt::ctx path (which commits to the prev value).
pub fn p2pkh_input_sighash(
    tx: &Transaction,
    input_index: usize,
    prev_value: i64,
    prev_script: &[u8],
) -> Result<[u8; 32], String> {
    let outpoints: Vec<Vec<u8>> = tx
        .inputs
        .iter()
        .map(|i| i.prev_out.serialize().map_err(|e| e.to_string()))
        .collect::<Result<_, _>>()?;
    let sequences: Vec<Vec<u8>> = tx.inputs.iter().map(|i| i.sequence.to_le_bytes().to_vec()).collect();
    let outs: Vec<(Vec<u8>, Vec<u8>)> = tx
        .outputs
        .iter()
        .map(|o| (o.value.to_le_bytes().to_vec(), o.script_pubkey.clone()))
        .collect();

    let header = ctx_header(
        &tx.version.to_le_bytes(),
        &hash_prevouts(&outpoints),
        &hash_sequence(&sequences),
        &outpoints[input_index],
    );
    let footer = ctx_footer(
        &prev_value.to_le_bytes(),
        &sequences[input_index],
        &hash_outputs(&outs),
        &tx.lock_time.to_le_bytes(),
        &[0x41, 0x00, 0x00, 0x00], // SIGHASH_ALL|FORKID, 4-byte LE
    );
    let preimage = build_preimage(&header, prev_script, &footer);
    Ok(preimage_sighash(&preimage))
}

/// Build + sign an MSBBolt genesis mint to `owner_pubkey` carrying `balance`,
/// funded by `funding` (self-funded: funder may be the owner), with `fee` sats.
pub fn build_msbbolt_mint(
    owner_pubkey: &[u8],
    balance: &[u8],
    funding: &Funding,
    fee: i64,
) -> Result<MintResult, String> {
    let bolt_lock = fill_locking_script(&min_simple_balance_bolt(), &mint_lock_args(owner_pubkey, balance))?;
    let funder_h160 = hash160(&funding.pub_key);
    let funder_p2pkh = Script::p2pkh_locking_script(&funder_h160).map_err(|e| e.to_string())?.bytes;

    let change_value = funding.value - 1 - fee;
    if change_value < 0 {
        return Err(format!("insufficient funding: {} < {}", funding.value, 1 + fee));
    }

    let mut tx = Transaction::new();
    tx.version = 2;
    tx.lock_time = 0;
    let mut input = TxInput::new(OutPoint::new(funding.txid.clone(), funding.vout));
    input.sequence = 0xffff_ffff;
    tx.add_input(input);
    tx.add_output(TxOutput::new(1, bolt_lock));
    tx.add_output(TxOutput::new(change_value, funder_p2pkh.clone()));

    let sighash = p2pkh_input_sighash(&tx, 0, funding.value, &funder_p2pkh)?;
    let sig = sign_sighash(&sighash, &funding.priv_key)?;
    let unlock = Script::p2pkh_unlocking_script(&sig, &funding.pub_key).bytes;
    tx.inputs[0].set_script(unlock);

    Ok(MintResult {
        raw_tx_hex: tx.to_hex().map_err(|e| e.to_string())?,
        txid: tx.txid().map_err(|e| e.to_string())?,
        bolt_vout: 0,
    })
}

/// Build + sign a MinSimpleBolt (plain NFT identity) genesis mint to
/// `owner_pubkey`, funded by `funding`, with `fee` sats. Mirrors
/// `build_msbbolt_mint` but the bolt output carries NO value field (identity
/// NFT, not a balance token). This is the wallet capability B1 needs.
pub fn build_min_simple_bolt_mint(
    owner_pubkey: &[u8],
    funding: &Funding,
    fee: i64,
) -> Result<MintResult, String> {
    let bolt_lock = fill_locking_script(
        &min_simple_bolt(),
        &super::min_simple_bolt::mint_lock_args(owner_pubkey),
    )?;
    let funder_h160 = hash160(&funding.pub_key);
    let funder_p2pkh = Script::p2pkh_locking_script(&funder_h160).map_err(|e| e.to_string())?.bytes;

    let change_value = funding.value - 1 - fee;
    if change_value < 0 {
        return Err(format!("insufficient funding: {} < {}", funding.value, 1 + fee));
    }

    let mut tx = Transaction::new();
    tx.version = 2;
    tx.lock_time = 0;
    let mut input = TxInput::new(OutPoint::new(funding.txid.clone(), funding.vout));
    input.sequence = 0xffff_ffff;
    tx.add_input(input);
    tx.add_output(TxOutput::new(1, bolt_lock));
    tx.add_output(TxOutput::new(change_value, funder_p2pkh.clone()));

    let sighash = p2pkh_input_sighash(&tx, 0, funding.value, &funder_p2pkh)?;
    let sig = sign_sighash(&sighash, &funding.priv_key)?;
    let unlock = Script::p2pkh_unlocking_script(&sig, &funding.pub_key).bytes;
    tx.inputs[0].set_script(unlock);

    Ok(MintResult {
        raw_tx_hex: tx.to_hex().map_err(|e| e.to_string())?,
        txid: tx.txid().map_err(|e| e.to_string())?,
        bolt_vout: 0,
    })
}

/// Build + sign a SimpleMultiBolt genesis mint of `balance` (16-byte LE pennies) owned by
/// `owner_pubkey`, funded by `funding`. Models a £IRP payment: minting an SMB of the amount
/// to the payee (the demo's bank does the same for loans). Verified by `lineage:'skip'` SPV.
pub fn build_smb_mint(
    owner_pubkey: &[u8],
    balance: &[u8],
    funding: &Funding,
    fee: i64,
) -> Result<MintResult, String> {
    let bolt_lock = fill_locking_script(
        &simple_multi_bolt(),
        &super::simple_multi_bolt::mint_lock_args(owner_pubkey, balance),
    )?;
    let funder_h160 = hash160(&funding.pub_key);
    let funder_p2pkh = Script::p2pkh_locking_script(&funder_h160).map_err(|e| e.to_string())?.bytes;

    let change_value = funding.value - 1 - fee;
    if change_value < 0 {
        return Err(format!("insufficient funding: {} < {}", funding.value, 1 + fee));
    }

    let mut tx = Transaction::new();
    tx.version = 2;
    tx.lock_time = 0;
    let mut input = TxInput::new(OutPoint::new(funding.txid.clone(), funding.vout));
    input.sequence = 0xffff_ffff;
    tx.add_input(input);
    tx.add_output(TxOutput::new(1, bolt_lock));
    tx.add_output(TxOutput::new(change_value, funder_p2pkh.clone()));

    let sighash = p2pkh_input_sighash(&tx, 0, funding.value, &funder_p2pkh)?;
    let sig = sign_sighash(&sighash, &funding.priv_key)?;
    let unlock = Script::p2pkh_unlocking_script(&sig, &funding.pub_key).bytes;
    tx.inputs[0].set_script(unlock);

    Ok(MintResult {
        raw_tx_hex: tx.to_hex().map_err(|e| e.to_string())?,
        txid: tx.txid().map_err(|e| e.to_string())?,
        bolt_vout: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bolt::sign::verify_sig;
    use secp256k1::{PublicKey, Secp256k1, SecretKey};

    #[test]
    fn builds_signed_msbbolt_mint() {
        let secp = Secp256k1::new();
        let sk = [0x11u8; 32];
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap())
            .serialize()
            .to_vec();
        let balance = hex::decode("000000000000000000000000000186a0").unwrap();

        let funding = Funding {
            txid: "aa".repeat(32),
            vout: 0,
            value: 100_000,
            priv_key: sk,
            pub_key: pk.clone(),
        };

        let r = build_msbbolt_mint(&pk, &balance, &funding, 200).unwrap();
        assert_eq!(r.txid.len(), 64);
        assert!(!r.raw_tx_hex.is_empty());

        // re-parse: bolt output[0] is the MSBBolt lock; verify the funding sig is valid
        let expected_lock = fill_locking_script(&min_simple_balance_bolt(), &mint_lock_args(&pk, &balance)).unwrap();
        assert!(r.raw_tx_hex.contains(&hex::encode(&expected_lock)), "bolt lock present in tx");

        // reconstruct the tx (without the signed input script) to recompute the sighash,
        // then verify the signature embedded in the built tx
        let mut unsigned = Transaction::new();
        unsigned.version = 2;
        unsigned.lock_time = 0;
        let mut inp = TxInput::new(OutPoint::new(funding.txid.clone(), funding.vout));
        inp.sequence = 0xffff_ffff;
        unsigned.add_input(inp);
        unsigned.add_output(TxOutput::new(1, expected_lock));
        let funder_p2pkh = Script::p2pkh_locking_script(&hash160(&pk)).unwrap().bytes;
        unsigned.add_output(TxOutput::new(100_000 - 1 - 200, funder_p2pkh.clone()));
        let sighash = p2pkh_input_sighash(&unsigned, 0, 100_000, &funder_p2pkh).unwrap();

        // pull the sig (first push) out of the built tx's input scriptSig
        let raw = hex::decode(&r.raw_tx_hex).unwrap();
        // locate the funder pubkey push to bound the scriptSig sig; simpler: re-sign deterministically and compare
        let sig = sign_sighash(&sighash, &sk).unwrap();
        assert!(verify_sig(&sig, &sighash, &pk).unwrap(), "funding sig verifies");
        assert!(raw.windows(sig.len()).any(|w| w == sig.as_slice()), "tx carries the funding signature");
    }

    /// Emit a deterministic B1 identity-mint fixture for the cross-repo
    /// acceptance test (spv-demo-wapps drives catpicz/bwanq with this rawTx).
    /// Writes {rawTxHex, txid, pubKeyHex, pubKeyHashHex, contract}; deterministic
    /// so it doubles as a committed fixture. Identity is a plain MinSimpleBolt NFT.
    #[test]
    fn emit_b1_identity_mint_fixture() {
        use crate::bolt::lib::hash160;
        let secp = Secp256k1::new();
        let sk = [0x2au8; 32]; // the wallet's user key (deterministic for the demo)
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap())
            .serialize()
            .to_vec();
        let funding = Funding {
            txid: "11".repeat(32),
            vout: 0,
            value: 100_000,
            priv_key: sk,
            pub_key: pk.clone(),
        };
        let r = build_min_simple_bolt_mint(&pk, &funding, 200).unwrap();
        let pkh = hash160(&pk);
        let json = format!(
            "{{\n  \"rawTxHex\": \"{}\",\n  \"txid\": \"{}\",\n  \"pubKeyHex\": \"{}\",\n  \"pubKeyHashHex\": \"{}\",\n  \"contract\": \"MinSimpleBolt\"\n}}\n",
            r.raw_tx_hex, r.txid, hex::encode(&pk), hex::encode(pkh)
        );
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures");
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(format!("{dir}/b1_identity_mint.json"), json).unwrap();
    }

    #[test]
    fn rejects_insufficient_funding() {
        let secp = Secp256k1::new();
        let sk = [0x22u8; 32];
        let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap()).serialize().to_vec();
        let funding = Funding { txid: "bb".repeat(32), vout: 1, value: 100, priv_key: sk, pub_key: pk.clone() };
        let balance = vec![0u8; 16];
        assert!(build_msbbolt_mint(&pk, &balance, &funding, 200).is_err());
    }
}
