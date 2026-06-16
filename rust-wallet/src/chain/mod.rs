//! ChainBackend: runtime-selectable chain access for BOLT + BSV.
//!
//! Layer D (BOLT-into-Hodos plan). Selects where the wallet broadcasts,
//! fetches proofs, and tracks headers. Default first target is `local-node`
//! (SV regtest via RPC + mining, accepts non-standard BOLT scripts). Public
//! `testnet`/`main` go via hosted Arcade; `ttn` (teratestnet) is the network
//! proven to accept BOLT. `local-arcade:<url>` points at a local Arcade.
//!
//! D-1 scope: the enum + config resolution + capability flags. The concrete
//! Broadcaster/ProofSource/HeaderSource impls land in D-2/D-3.

pub mod arcade;
pub mod endpoints;
pub mod node;

/// Hosted Arcade base URLs (no trailing slash; routes are e.g. `/tx`).
const ARCADE_MAIN: &str = "https://arcade-v2-us-1.bsvblockchain.tech";
const ARCADE_TTN: &str = "https://arcade-v2-ttn-us-1.bsvblockchain.tech";
const ARCADE_TESTNET: &str = "https://arcade-v2-testnet-us-1.bsvblockchain.tech";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChainBackend {
    /// Local SV node (regtest) via JSON-RPC + mining. Default first target.
    LocalNode,
    /// Public BSV testnet via hosted Arcade.
    Testnet,
    /// Teratestnet via hosted Arcade (proven to accept BOLT).
    Ttn,
    /// Mainnet via hosted Arcade (BOLT ops disabled by policy).
    Mainnet,
    /// A locally-run Arcade instance at the given base URL.
    LocalArcade(String),
}

impl ChainBackend {
    /// Resolve a backend from a name token. `local-arcade:<url>` carries a URL.
    /// Returns `None` for an unrecognised name.
    pub fn from_name(name: &str) -> Option<ChainBackend> {
        let trimmed = name.trim();
        let n = trimmed.to_ascii_lowercase();
        if n.starts_with("local-arcade:") {
            let url = trimmed["local-arcade:".len()..].to_string(); // preserve URL case
            if url.is_empty() {
                return None;
            }
            return Some(ChainBackend::LocalArcade(url));
        }
        match n.as_str() {
            "local-node" | "localnode" | "local" | "regtest" => Some(ChainBackend::LocalNode),
            "testnet" | "test" => Some(ChainBackend::Testnet),
            "ttn" | "teratestnet" => Some(ChainBackend::Ttn),
            "main" | "mainnet" => Some(ChainBackend::Mainnet),
            _ => None,
        }
    }

    /// Resolve from an optional config/env value. `None` (unset) -> the default
    /// first target (`LocalNode`). An unrecognised value also falls back to the
    /// default so a typo can never silently select mainnet.
    pub fn from_opt(value: Option<&str>) -> ChainBackend {
        match value {
            Some(v) if !v.trim().is_empty() => {
                ChainBackend::from_name(v).unwrap_or(ChainBackend::LocalNode)
            }
            _ => ChainBackend::LocalNode,
        }
    }

    /// Resolve from the `HODOS_CHAIN` environment variable (default `LocalNode`).
    pub fn from_env() -> ChainBackend {
        ChainBackend::from_opt(std::env::var("HODOS_CHAIN").ok().as_deref())
    }

    /// Hosted/local Arcade base URL for Arcade-backed networks; `None` for
    /// `local-node` (which broadcasts via node RPC, not Arcade).
    pub fn arcade_base_url(&self) -> Option<&str> {
        match self {
            ChainBackend::Mainnet => Some(ARCADE_MAIN),
            ChainBackend::Ttn => Some(ARCADE_TTN),
            ChainBackend::Testnet => Some(ARCADE_TESTNET),
            ChainBackend::LocalArcade(url) => Some(url.as_str()),
            ChainBackend::LocalNode => None,
        }
    }

    /// Whether an address->UTXO indexer (WhatsOnChain) is available. Only the
    /// public BSV networks have one; ttn/local self-track outputs instead.
    pub fn has_address_indexer(&self) -> bool {
        matches!(self, ChainBackend::Testnet | ChainBackend::Mainnet)
    }

    /// Whether BOLT token ops are permitted. Disabled on mainnet (standard-
    /// script policy rejects BOLT's non-standard scripts).
    pub fn bolt_enabled(&self) -> bool {
        !matches!(self, ChainBackend::Mainnet)
    }

    /// Stable short name (round-trips through `from_name`).
    pub fn name(&self) -> String {
        match self {
            ChainBackend::LocalNode => "local-node".into(),
            ChainBackend::Testnet => "testnet".into(),
            ChainBackend::Ttn => "ttn".into(),
            ChainBackend::Mainnet => "main".into(),
            ChainBackend::LocalArcade(url) => format!("local-arcade:{url}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_name_resolves_known_networks() {
        assert_eq!(ChainBackend::from_name("local-node"), Some(ChainBackend::LocalNode));
        assert_eq!(ChainBackend::from_name("regtest"), Some(ChainBackend::LocalNode));
        assert_eq!(ChainBackend::from_name("testnet"), Some(ChainBackend::Testnet));
        assert_eq!(ChainBackend::from_name("TTN"), Some(ChainBackend::Ttn));
        assert_eq!(ChainBackend::from_name("teratestnet"), Some(ChainBackend::Ttn));
        assert_eq!(ChainBackend::from_name("main"), Some(ChainBackend::Mainnet));
        assert_eq!(ChainBackend::from_name("bogus"), None);
    }

    #[test]
    fn from_name_parses_local_arcade_url_preserving_case() {
        assert_eq!(
            ChainBackend::from_name("local-arcade:http://127.0.0.1:8080"),
            Some(ChainBackend::LocalArcade("http://127.0.0.1:8080".into()))
        );
        assert_eq!(ChainBackend::from_name("local-arcade:"), None);
    }

    #[test]
    fn from_opt_defaults_to_local_node() {
        assert_eq!(ChainBackend::from_opt(None), ChainBackend::LocalNode);
        assert_eq!(ChainBackend::from_opt(Some("")), ChainBackend::LocalNode);
        // unrecognised never silently selects mainnet
        assert_eq!(ChainBackend::from_opt(Some("typo")), ChainBackend::LocalNode);
        assert_eq!(ChainBackend::from_opt(Some("ttn")), ChainBackend::Ttn);
    }

    #[test]
    fn arcade_base_urls() {
        assert_eq!(ChainBackend::Mainnet.arcade_base_url(), Some(ARCADE_MAIN));
        assert_eq!(ChainBackend::Ttn.arcade_base_url(), Some(ARCADE_TTN));
        assert_eq!(ChainBackend::Testnet.arcade_base_url(), Some(ARCADE_TESTNET));
        assert_eq!(ChainBackend::LocalNode.arcade_base_url(), None);
        assert_eq!(
            ChainBackend::LocalArcade("http://x:8080".into()).arcade_base_url(),
            Some("http://x:8080")
        );
    }

    #[test]
    fn capabilities() {
        assert!(ChainBackend::Testnet.has_address_indexer());
        assert!(ChainBackend::Mainnet.has_address_indexer());
        assert!(!ChainBackend::Ttn.has_address_indexer());
        assert!(!ChainBackend::LocalNode.has_address_indexer());

        assert!(ChainBackend::LocalNode.bolt_enabled());
        assert!(ChainBackend::Ttn.bolt_enabled());
        assert!(ChainBackend::Testnet.bolt_enabled());
        assert!(!ChainBackend::Mainnet.bolt_enabled());
    }

    #[test]
    fn name_round_trips() {
        for b in [
            ChainBackend::LocalNode,
            ChainBackend::Testnet,
            ChainBackend::Ttn,
            ChainBackend::Mainnet,
            ChainBackend::LocalArcade("http://h:1".into()),
        ] {
            assert_eq!(ChainBackend::from_name(&b.name()), Some(b.clone()), "round-trip {b:?}");
        }
    }
}
