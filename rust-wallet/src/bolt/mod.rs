//! BOLT token engine - Rust port of the ts-bolt contract logic (Layer B).
//!
//! Embeds the compiled production-contract artifacts (frozen by the sx-side
//! Layer A' `artifactExport.test.js`) and fills them into scripts, mirroring the
//! sx simulator's `processRecombinants`. Per-contract arg builders + golden
//! byte-match tests land in B-2..C-3.

pub mod ctx;
pub mod lib;
pub mod min_simple_bolt;
pub mod msbbolt;
pub mod sx_template;
