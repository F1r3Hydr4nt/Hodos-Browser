//! D-4a: pure endpoint resolution for the existing broadcast / proof / height /
//! UTXO operations, keyed by `ChainBackend`. This is the routing TABLE only -
//! it decides, per backend, WHICH transport each operation uses and builds the
//! concrete URLs. The live handlers (`handlers.rs` broadcast_transaction,
//! cache_helpers proof fetch, get_height) consume this in D-4b so the production
//! money paths change in one small, reviewable step rather than a big-bang edit.
//!
//! Decision matrix (from the BOLT plan):
//!   - LocalNode            -> node JSON-RPC (sendrawtransaction / confirmation)
//!   - Ttn / LocalArcade    -> hosted/local Arcade (POST /tx, GET /tx/:txid BUMP)
//!   - Testnet / Mainnet    -> existing WhatsOnChain/ARC path (kept as today)
//! UTXO-by-address indexer exists ONLY for Testnet/Mainnet (WoC); ttn/local
//! self-track outputs instead.

use super::ChainBackend;

/// WhatsOnChain network segment for a backend's REST path (`/v1/bsv/<seg>/...`).
/// Only meaningful for the indexer-backed public networks.
pub fn woc_network_segment(backend: &ChainBackend) -> Option<&'static str> {
    match backend {
        ChainBackend::Mainnet => Some("main"),
        ChainBackend::Testnet => Some("test"),
        _ => None,
    }
}

/// Which transport carries a raw-tx broadcast for this backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BroadcastRoute {
    /// Local SV node JSON-RPC `sendrawtransaction`.
    NodeRpc,
    /// Arcade `POST <base>/tx` (octet-stream). URL is fully built.
    Arcade { url: String },
    /// Existing WhatsOnChain/ARC broadcast path (mainnet/testnet, unchanged).
    WocArc { woc_raw_url: String },
}

/// Which transport carries a merkle-proof / confirmation fetch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProofRoute {
    /// Node JSON-RPC tx confirmation (no BUMP; local mining builds it).
    NodeRpc,
    /// Arcade `GET <base>/tx/:txid` -> status incl. BUMP merklePath.
    Arcade { url: String },
    /// Existing WhatsOnChain TSC proof path (mainnet/testnet, unchanged).
    WocTsc { tsc_url: String },
}

impl ChainBackend {
    /// Resolve the broadcast transport for this backend.
    pub fn broadcast_route(&self) -> BroadcastRoute {
        match self {
            ChainBackend::LocalNode => BroadcastRoute::NodeRpc,
            ChainBackend::Ttn | ChainBackend::LocalArcade(_) => BroadcastRoute::Arcade {
                // arcade_base_url is Some for these variants
                url: format!("{}/tx", self.arcade_base_url().unwrap()),
            },
            ChainBackend::Testnet | ChainBackend::Mainnet => BroadcastRoute::WocArc {
                woc_raw_url: format!(
                    "https://api.whatsonchain.com/v1/bsv/{}/tx/raw",
                    woc_network_segment(self).unwrap()
                ),
            },
        }
    }

    /// Resolve the proof/confirmation transport for `txid`.
    pub fn proof_route(&self, txid: &str) -> ProofRoute {
        match self {
            ChainBackend::LocalNode => ProofRoute::NodeRpc,
            ChainBackend::Ttn | ChainBackend::LocalArcade(_) => ProofRoute::Arcade {
                url: format!("{}/tx/{}", self.arcade_base_url().unwrap(), txid),
            },
            ChainBackend::Testnet | ChainBackend::Mainnet => ProofRoute::WocTsc {
                tsc_url: format!(
                    "https://api.whatsonchain.com/v1/bsv/{}/tx/{}/proof/tsc",
                    woc_network_segment(self).unwrap(),
                    txid
                ),
            },
        }
    }

    /// WhatsOnChain chain-info URL for the current height, or `None` when this
    /// backend has no WoC indexer (ttn/local resolve height via node/arcade).
    pub fn woc_chain_info_url(&self) -> Option<String> {
        woc_network_segment(self)
            .map(|seg| format!("https://api.whatsonchain.com/v1/bsv/{seg}/chain/info"))
    }

    /// WhatsOnChain address-UTXO URL, or `None` when no address indexer is
    /// available (gate caller on this == `has_address_indexer`).
    pub fn woc_address_unspent_url(&self, address: &str) -> Option<String> {
        woc_network_segment(self).map(|seg| {
            format!("https://api.whatsonchain.com/v1/bsv/{seg}/address/{address}/unspent/all")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn broadcast_route_per_backend() {
        assert_eq!(ChainBackend::LocalNode.broadcast_route(), BroadcastRoute::NodeRpc);
        assert_eq!(
            ChainBackend::Ttn.broadcast_route(),
            BroadcastRoute::Arcade {
                url: "https://arcade-v2-ttn-us-1.bsvblockchain.tech/tx".into()
            }
        );
        assert_eq!(
            ChainBackend::LocalArcade("http://127.0.0.1:8080".into()).broadcast_route(),
            BroadcastRoute::Arcade { url: "http://127.0.0.1:8080/tx".into() }
        );
        assert_eq!(
            ChainBackend::Mainnet.broadcast_route(),
            BroadcastRoute::WocArc {
                woc_raw_url: "https://api.whatsonchain.com/v1/bsv/main/tx/raw".into()
            }
        );
        assert_eq!(
            ChainBackend::Testnet.broadcast_route(),
            BroadcastRoute::WocArc {
                woc_raw_url: "https://api.whatsonchain.com/v1/bsv/test/tx/raw".into()
            }
        );
    }

    #[test]
    fn proof_route_per_backend() {
        let txid = "abc123";
        assert_eq!(ChainBackend::LocalNode.proof_route(txid), ProofRoute::NodeRpc);
        assert_eq!(
            ChainBackend::Ttn.proof_route(txid),
            ProofRoute::Arcade {
                url: "https://arcade-v2-ttn-us-1.bsvblockchain.tech/tx/abc123".into()
            }
        );
        assert_eq!(
            ChainBackend::Mainnet.proof_route(txid),
            ProofRoute::WocTsc {
                tsc_url: "https://api.whatsonchain.com/v1/bsv/main/tx/abc123/proof/tsc".into()
            }
        );
        assert_eq!(
            ChainBackend::Testnet.proof_route(txid),
            ProofRoute::WocTsc {
                tsc_url: "https://api.whatsonchain.com/v1/bsv/test/tx/abc123/proof/tsc".into()
            }
        );
    }

    #[test]
    fn woc_urls_only_for_indexed_networks() {
        assert_eq!(
            ChainBackend::Mainnet.woc_chain_info_url().as_deref(),
            Some("https://api.whatsonchain.com/v1/bsv/main/chain/info")
        );
        assert_eq!(
            ChainBackend::Testnet.woc_chain_info_url().as_deref(),
            Some("https://api.whatsonchain.com/v1/bsv/test/chain/info")
        );
        assert_eq!(ChainBackend::Ttn.woc_chain_info_url(), None);
        assert_eq!(ChainBackend::LocalNode.woc_chain_info_url(), None);

        assert_eq!(
            ChainBackend::Mainnet.woc_address_unspent_url("1ADDR").as_deref(),
            Some("https://api.whatsonchain.com/v1/bsv/main/address/1ADDR/unspent/all")
        );
        // address-UTXO URL is available iff the backend has an address indexer
        for b in [
            ChainBackend::LocalNode,
            ChainBackend::Testnet,
            ChainBackend::Ttn,
            ChainBackend::Mainnet,
            ChainBackend::LocalArcade("http://h:1".into()),
        ] {
            assert_eq!(
                b.woc_address_unspent_url("X").is_some(),
                b.has_address_indexer(),
                "utxo url availability tracks has_address_indexer for {b:?}"
            );
        }
    }

    #[test]
    fn woc_segment_only_public_networks() {
        assert_eq!(woc_network_segment(&ChainBackend::Mainnet), Some("main"));
        assert_eq!(woc_network_segment(&ChainBackend::Testnet), Some("test"));
        assert_eq!(woc_network_segment(&ChainBackend::Ttn), None);
        assert_eq!(woc_network_segment(&ChainBackend::LocalNode), None);
    }
}
