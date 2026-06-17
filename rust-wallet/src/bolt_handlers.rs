//! BOLT token HTTP endpoints (Layer E) — let browser pages drive the wallet's BOLT engine.
//!
//! `/bolt/identity/mint` self-issues a MinSimpleBolt identity NFT (plain NFT, no value) owned
//! by the wallet's demo identity key and returns its signed raw tx. The page then proves it
//! via the shared chain service (`POST /broadcast`) and presents the resulting BEEF to a site
//! (e.g. catpicz `/identity/verify`) for SPV verification + account binding.
//!
//! Hermetic-demo scope: a deterministic single-user identity key (re-minting yields the same
//! owner pkh, so a site recognises the returning user). No DB / key-derivation changes.

use actix_web::{web, HttpResponse};
use secp256k1::{PublicKey, Secp256k1, SecretKey};
use serde::{Deserialize, Serialize};

use crate::bolt::lib::hash160;
use crate::bolt::mint::{build_min_simple_bolt_mint, build_smb_mint, Funding};
use crate::bolt::sign::sign_sighash;
use crate::crypto::signing::sha256;

/// Deterministic demo identity key (matches the committed `b1_identity_mint.json` fixture).
const DEMO_IDENTITY_SK: [u8; 32] = [0x2a; 32];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityMintResponse {
    pub raw_tx_hex: String,
    pub txid: String,
    pub pub_key_hex: String,
    pub pub_key_hash_hex: String,
    pub contract: &'static str,
}

fn demo_identity_pubkey() -> Vec<u8> {
    let secp = Secp256k1::new();
    let sk = SecretKey::from_slice(&DEMO_IDENTITY_SK).expect("valid demo sk");
    PublicKey::from_secret_key(&secp, &sk).serialize().to_vec()
}

/// A real funding UTXO the demo identity key controls (regtest/testnet). `txid` is the
/// standard display txid (the wallet reverses it for the wire prevout).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FundingInput {
    pub txid: String,
    pub vout: u32,
    pub value: i64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MintReqBody {
    /// When present, the mint spends this real UTXO (regtest). Absent → hermetic seed funding.
    pub funding: Option<FundingInput>,
}

/// Build (sign) a MinSimpleBolt identity mint owned by the wallet's demo identity key. With a
/// `funding_override` it spends a REAL UTXO (regtest broadcast); without, it uses the hermetic
/// seed outpoint (proven by the chain service's synthetic BUMP).
pub fn build_demo_identity_mint(
    funding_override: Option<FundingInput>,
) -> Result<IdentityMintResponse, String> {
    let pk = demo_identity_pubkey();
    let funding = match funding_override {
        Some(f) => Funding {
            txid: f.txid,
            vout: f.vout,
            value: f.value,
            priv_key: DEMO_IDENTITY_SK,
            pub_key: pk.clone(),
        },
        None => Funding {
            txid: "11".repeat(32),
            vout: 0,
            value: 100_000,
            priv_key: DEMO_IDENTITY_SK,
            pub_key: pk.clone(),
        },
    };
    let r = build_min_simple_bolt_mint(&pk, &funding, 200)?;
    Ok(IdentityMintResponse {
        raw_tx_hex: r.raw_tx_hex,
        txid: r.txid,
        pub_key_hex: hex::encode(&pk),
        pub_key_hash_hex: hex::encode(hash160(&pk)),
        contract: "MinSimpleBolt",
    })
}

/// POST /bolt/identity/mint — mint the identity NFT, return the signed raw tx for proving.
/// Optional JSON body `{ "funding": {txid,vout,value} }` for a real UTXO (empty body → seed).
pub async fn bolt_identity_mint(body: web::Bytes) -> HttpResponse {
    let funding = if body.is_empty() {
        None
    } else {
        match serde_json::from_slice::<MintReqBody>(&body) {
            Ok(b) => b.funding,
            Err(e) => {
                return HttpResponse::BadRequest()
                    .json(serde_json::json!({ "error": format!("bad request body: {e}") }))
            }
        }
    };
    match build_demo_identity_mint(funding) {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({ "error": e })),
    }
}

/// GET /bolt/identity — the wallet's demo identity pubkey/pkh (no mint).
pub async fn bolt_identity() -> HttpResponse {
    let pk = demo_identity_pubkey();
    HttpResponse::Ok().json(serde_json::json!({
        "pubKeyHex": hex::encode(&pk),
        "pubKeyHashHex": hex::encode(hash160(&pk)),
        "contract": "MinSimpleBolt",
    }))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PayRequest {
    pub pay_to_pub_key_hex: String,
    pub amount: u64, // minor units (pennies)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PayResponse {
    pub raw_tx_hex: String,
    pub txid: String,
    pub pay_to_pub_key_hash_hex: String,
    pub amount: u64,
    pub contract: &'static str,
}

/// Build a SimpleMultiBolt payment of `amount` pennies owned by `pay_to_pub_key_hex` (the
/// merchant), funded by the wallet's demo identity key. Models paying from an IRP balance —
/// the demo bank mints SMB the same way; verified by the site under `lineage:'skip'` SPV.
pub fn build_demo_payment(pay_to_pub_key_hex: &str, amount: u64) -> Result<PayResponse, String> {
    let pay_to = hex::decode(pay_to_pub_key_hex).map_err(|e| format!("payToPubKeyHex: {e}"))?;
    if pay_to.len() != 33 {
        return Err("payToPubKeyHex must be a 33-byte compressed pubkey".into());
    }
    let balance = (amount as u128).to_le_bytes().to_vec(); // 16-byte LE (matches amount16)
    let funder_pk = demo_identity_pubkey();
    let funding = Funding {
        txid: "22".repeat(32),
        vout: 0,
        value: 1_000_000,
        priv_key: DEMO_IDENTITY_SK,
        pub_key: funder_pk,
    };
    let r = build_smb_mint(&pay_to, &balance, &funding, 200)?;
    Ok(PayResponse {
        raw_tx_hex: r.raw_tx_hex,
        txid: r.txid,
        pay_to_pub_key_hash_hex: hex::encode(hash160(&pay_to)),
        amount,
        contract: "SimpleMultiBolt",
    })
}

/// POST /bolt/pay {payToPubKeyHex, amount} — mint an SMB payment, return its raw tx.
pub async fn bolt_pay(body: web::Json<PayRequest>) -> HttpResponse {
    match build_demo_payment(&body.pay_to_pub_key_hex, body.amount) {
        Ok(resp) => HttpResponse::Ok().json(resp),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e })),
    }
}

#[derive(Deserialize)]
pub struct SignRequest {
    pub message: String,
}

/// Sign `message` (UTF-8) with the wallet's demo identity key → (DER hex, pubKeyHex).
/// ECDSA over single sha256(message) — matches `@bsv` PublicKey.verify(msg, sig, 'utf8'),
/// reusing the audited `sign_sighash` path (low-S) and stripping its sighash-type byte.
pub fn sign_offer(message: &str) -> Result<(String, String), String> {
    let digest_vec = sha256(message.as_bytes());
    let digest: [u8; 32] = digest_vec.as_slice().try_into().map_err(|_| "sha256 length")?;
    let mut der = sign_sighash(&digest, &DEMO_IDENTITY_SK)?;
    der.pop(); // strip the appended sighash-type byte → plain ECDSA DER for message verify
    Ok((hex::encode(&der), hex::encode(demo_identity_pubkey())))
}

/// POST /bolt/sign {message} — sign an offer message (for bucket listings, B7).
pub async fn bolt_sign(body: web::Json<SignRequest>) -> HttpResponse {
    match sign_offer(&body.message) {
        Ok((signature, pub_key_hex)) => HttpResponse::Ok().json(serde_json::json!({
            "signature": signature,
            "pubKeyHex": pub_key_hex,
        })),
        Err(e) => HttpResponse::BadRequest().json(serde_json::json!({ "error": e })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_identity_mint_is_minsimplebolt_owned_by_demo_key() {
        let r = build_demo_identity_mint(None).unwrap();
        assert_eq!(r.contract, "MinSimpleBolt");
        assert_eq!(r.txid.len(), 64);
        // owner pkh matches the committed b1 fixture identity and is embedded in the lock
        assert_eq!(r.pub_key_hash_hex, "1e3ea287c5ea24394daa072ca817859103cafc71");
        assert!(r.raw_tx_hex.contains(&r.pub_key_hash_hex), "lock embeds owner pkh");
    }

    #[test]
    fn demo_payment_is_smb_owned_by_payee() {
        let merchant = format!("02{}", "ab".repeat(32)); // 33-byte compressed pubkey shape
        let r = build_demo_payment(&merchant, 4500).unwrap();
        assert_eq!(r.contract, "SimpleMultiBolt");
        assert_eq!(r.amount, 4500);
        assert_eq!(r.txid.len(), 64);
        assert!(r.raw_tx_hex.contains(&r.pay_to_pub_key_hash_hex), "lock embeds payee pkh");
    }

    #[test]
    fn sign_offer_returns_der_for_identity_key() {
        let (sig_hex, pk_hex) = sign_offer("bucket.shop:list:abc:333").unwrap();
        assert_eq!(pk_hex, "035be5e9478209674a96e60f1f037f6176540fd001fa1d64694770c56a7709c42c");
        let der = hex::decode(&sig_hex).unwrap();
        assert!(der.len() > 8 && der[0] == 0x30, "ECDSA DER (no sighash byte)");
    }
}
