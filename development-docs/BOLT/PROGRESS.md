# BOLT support integration - ralph-loop PROGRESS

Loop state for the BOLT-into-Hodos build. Full plan: `development-docs/BOLT/BOLT-integration-plan.md`.
Drive Layers A to F to green (Layer G app tests are OUT OF SCOPE - apps do not exist yet).

## Per-iteration protocol
1. Pick the first unchecked, unblocked task whose dependencies (in brackets) are all checked.
2. RED: write its named failing test first.
3. GREEN: implement the minimum to pass.
4. Run that task's layer suite (commands below).
5. Commit (one commit per green task) on branch `bolt-integration` in the correct repo.
6. Tick the box here. Stop when every box is ticked and Layers A,A',B,C,D,E,F are green.

## Environment (READY - verified 2026-06-16)
- Rust 1.96 + Windows SDK installed. Bash shims at `~/bin/{cargo,rustc,rustup}` auto-load the VS env (`~/.vsenv.sh`), so plain `cargo`/`cargo test` work in any fresh shell. `rust-wallet` `cargo check` passes.
- Rust work lives in `Hodos-Browser/rust-wallet` (repo: Hodos-Browser). sx/ts-bolt work lives in the parent `priv-chain` repo. Commit to whichever repo the changed files belong to. Both repos are on branch `bolt-integration`.
- Test commands: Rust `cd Hodos-Browser/rust-wallet && cargo test <name>`; sx/node `cd priv-chain && npx jest <pattern>`.

## Guardrails (hard stops)
- Never push without explicit approval. Commit only.
- PAUSE for a human before editing `rust-wallet/src/crypto/`, signing/derivation, or any DB schema/migration (Hodos invariants).
- If a task cannot go green in a few attempts, mark it `BLOCKED: <first byte-diff or error>` and move to the next independent task.
- Do NOT create Layer-G work (real demo sites, issuer backends, Playwright `bolt-demo/test/demo.pw.ts`).

## Backlog

### Track A - Chain infra (parallel with Track B)
- [ ] D-1 ChainBackend enum + from_settings resolves local-node/testnet/ttn/main (+ HODOS_CHAIN); Broadcaster/ProofSource/HeaderSource traits - unit test
- [ ] D-2a node.rs (default): SV-node RPC sendrawtransaction + generatetoaddress + recorded-response test [D-1]
- [ ] D-2b arcade.rs: broadcast POST /tx + error-mapping test [D-1]
- [ ] D-3 proof: node getrawtransaction/mined-confirmation AND arcade get_tx_status BUMP merklePath fed to beef.rs - test [D-1]
- [ ] D-4 route existing broadcast/proof/height through ChainBackend; gate WoC UTXO on has_address_indexer [D-2a,D-2b,D-3]
- [ ] D-5 self-track outputs + POST /wallet/import-funding - test [D-4]

### Track B - Contract engine
- [x] Aprime-1 artifactExport.test.js freezes the 7 compiled production artifacts to **sx/bolt/production/artifacts/*.json** (drift-gated by recompilation; LF-pinned). DONE priv-chain `22c8260`. NOTE: emitted in priv-chain (co-located, single-repo) not rust-wallet; B-0 copies/embeds them into rust-wallet. Each artifact has filename/lockArgs/unlockArgs/lockOps/unlockOps/locking+unlockingRecombinants (lockOps present, e.g. MinSimpleBolt lockOps[855]/unlockOps[380]).
- [ ] B-0 sx_template.rs filler + lib.rs boltLib helpers; MinSimpleBolt lock-script golden match [Aprime-1]
- [ ] B-1 emit-golden.mjs emits lifecycle fixtures for MinSimpleBolt, MSBBolt, SimpleMultiBolt, Discount [Aprime-1, priv-chain repo]
- [ ] B-2 MinSimpleBolt mint/transfer/melt - golden byte-match [B-0,B-1]
- [ ] B-3 MinSimpleBalanceBolt mint/transfer/self-transfer - golden byte-match [B-2]
- [ ] C-1 MinSimpleBolt + MSBBolt forgery parity (verifyTx rejects) [B-3]
- [ ] B-4 SimpleMultiBolt mint/transfer/split/merge/melt - golden byte-match [B-3]
- [ ] C-2 SimpleMultiBolt forgery parity [B-4]
- [ ] B-5 MinSimpleDiscountBolt mint/transfer/melt - golden byte-match [B-3]
- [ ] C-3 Discount forgery parity [B-5]

### Track C - Wallet surface [needs D-4 + B-4 + B-5]
- [ ] E-1 /bolt/{mint,transfer,melt,receive} handlers per contract + tests
- [ ] E-2 GET /bolt/proof/:id BEEF export verifies via smb-payments
- [ ] F-1 inject window.hodosBrowser.bolt.* + domain-perm routing - test
- [ ] F-2 bolt-demo/stub/ page drives the full provider loop - test
- [ ] F-3 auto-register/account binding + toasts; useBolt.ts + TokensTab.tsx

## Completion gate
Every box ticked AND suites A, A', B, C, D, E, F green AND no Layer-G artifact created.

## Log
- 2026-06-16 env bring-up complete (rustup + Windows SDK + vsenv shims); branched bolt-integration in both repos; PROGRESS seeded.
- 2026-06-16 Aprime-1 GREEN (7/7): production artifacts frozen + LF-pinned in priv-chain (`22c8260`). Next eligible: D-1 (Track A) or B-0 (needs Aprime-1, embeds artifacts into rust-wallet).
