//! Emit a Hodos-wallet MSBBolt identity mint for a runtime funding UTXO, as JSON
//! on stdout. Used by the (B) local-regtest acceptance orchestration: fund a key
//! on the node, run this to get the signed mint rawTx, broadcast it, verify at
//! catpicz. Not part of the wallet binary (an example), so the dev-safeguard in
//! main() does not apply.
//!
//! Env:
//!   FUND_TXID   hex txid of the funding UTXO (required)
//!   FUND_VOUT   output index (default 0)
//!   FUND_VALUE  satoshis (required)
//!   OWNER_SK    32-byte hex private key (default 2a*32 — owner == funder)
//!   BALANCE     16-byte hex identity balance (default ...cafe)
//!   FEE         fee sats (default 200)

use hodos_wallet::bolt::lib::hash160;
use hodos_wallet::bolt::mint::{build_msbbolt_mint, Funding};
use secp256k1::{PublicKey, Secp256k1, SecretKey};

fn env(k: &str, default: &str) -> String {
    std::env::var(k).unwrap_or_else(|_| default.to_string())
}

fn main() {
    let sk_hex = env("OWNER_SK", &"2a".repeat(32));
    let sk: [u8; 32] = hex::decode(&sk_hex).expect("OWNER_SK hex").try_into().expect("32 bytes");
    let secp = Secp256k1::new();
    let pk = PublicKey::from_secret_key(&secp, &SecretKey::from_slice(&sk).unwrap())
        .serialize()
        .to_vec();

    let balance = hex::decode(env("BALANCE", "0000000000000000000000000000cafe")).expect("BALANCE hex");
    let funding = Funding {
        txid: std::env::var("FUND_TXID").expect("FUND_TXID required"),
        vout: env("FUND_VOUT", "0").parse().expect("FUND_VOUT u32"),
        value: std::env::var("FUND_VALUE").expect("FUND_VALUE required").parse().expect("FUND_VALUE i64"),
        priv_key: sk,
        pub_key: pk.clone(),
    };
    let fee: i64 = env("FEE", "200").parse().expect("FEE i64");

    let r = build_msbbolt_mint(&pk, &balance, &funding, fee).expect("build mint");
    println!(
        "{{\"rawTxHex\":\"{}\",\"txid\":\"{}\",\"pubKeyHashHex\":\"{}\",\"pubKeyHex\":\"{}\"}}",
        r.raw_tx_hex,
        r.txid,
        hex::encode(hash160(&pk)),
        hex::encode(&pk)
    );
}
