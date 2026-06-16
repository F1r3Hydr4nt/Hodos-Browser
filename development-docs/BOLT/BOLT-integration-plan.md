# Integrate the BOLT Token Family into Hodos — TDD, Demo-Driven

## Context

Hodos Browser ships a production BSV wallet: React UI → C++ CEF shell (intercepts `localhost:31301`) → **Rust** wallet daemon (Actix-web + SQLite). Hard invariant: *private keys never leave Rust; all signing in Rust*. A placeholder `frontend/src/components/wallet/TokensTab.tsx` awaits content.

The sibling `priv-chain` repo contains a working **BOLT** token protocol: a TS SDK (`ts-bolt/`), a Node wallet (`bolt-wallet/`), an SPV lib (`smb-payments/`), and precompiled `.sx.json` contracts — including three production minimal variants the demo needs.

**Goal:** demonstrate the BOLT token family interacting with real web services *through the browser wallet*, built **test-first**. The demo is the spec; every capability is justified by a demo beat and gated by a test.

### Decisions (with the user)
1. **User wallet ops = Rust-native port** into `rust-wallet` (keys stay in Rust, one daemon). C++/React gain `/bolt/*` calls like existing wallet endpoints.
2. **Demo sites = external issuers** running Node `ts-bolt` (they are separate parties; legitimately hold their own issuer keys). This shrinks the Rust surface for the demo and is realistic.
3. **Network = configurable `ChainBackend`** over Arcade: `main`/`ttn`/`testnet` (hosted `arcade-v2-*.bsvblockchain.tech`) + `local` Docker override. Default **`ttn`**. BOLT ops disabled on `main` (mainnet policy rejects non-standard scripts).
4. **dApp ↔ wallet via an injected provider** `window.hodosBrowser.bolt.*` (+ existing domain-permission/auto-approve + an approval overlay + celebratory toasts).
5. **TDD throughout**: each phase is Red (write failing test) → Green (implement) → Refactor.

### Demo contracts (already exist in `priv-chain/sx/bolt/production/`)
- **MinimumSimpleBalanceBolt (MSBBolt)** — NFT + immutable 16-byte balance → *identity / proof-of-activity* token.
- **SimpleMultiBolt** — fungible 16-byte balance + `currency` → *loan money (£ IRP)*.
- **MinSimpleDiscountBolt** — NFT + immutable 1-byte percent (1–100) → *discount coupons*.

---

## North-Star Demo (acceptance scenario)

| # | Beat | Capability | Contract | Issuer | Test |
|---|------|-----------|----------|--------|------|
| B1 | Visit **catpicz.xyz**, need account → **mint** an identity token to `usersChosenPubKey`, **send SPV proof** to the site | wallet mint + SPV export; site verifies | MSBBolt | **user wallet** (self-issued) | E2E B1 + SPV-verify unit |
| B2 | Visit **bwanq.xxx**, reuse identity by **send-to-self** as proof of activity/ownership, send SPV proof, **auto-register to account1** | self-transfer + SPV proof + browser account binding + auto-approve | MSBBolt | user wallet | E2E B2 |
| B3 | Bank approves **£100.00 IRP** loan → **minted into a SimpleMultiBolt** to `userChosenPubKey`; wallet receives + shows currency balance | receive issuer-minted fungible; currency display | SimpleMultiBolt | **bwanq (Node issuer)** | E2E B3 + receive/decode unit |
| B4 | Visit **tackle.new**, browser **auto-creates account**; 10% coupon shown → **redeem** → coupon lands in wallet → **toasts** celebrate | receive issuer mint+transfer; toast UX | MinSimpleDiscountBolt | **tackle (Node issuer)** | E2E B4 |
| B5 | Buy a fishing rod, **apply the 10% coupon**; pay with IRP | fungible transfer (pay) + coupon consumption (melt/redeem) | SimpleMultiBolt + Discount | user wallet → tackle | E2E B5 + transfer/melt unit |
| B6 | tackle.new mints + transfers a **further 5% coupon** to the user | receive issuer mint+transfer | MinSimpleDiscountBolt | tackle (Node issuer) | E2E B6 |
| B7 | Visit **bucket.shop** (exchange), auto-register, **list the 5% coupon for sale for £3.33** | list/offer a token for sale | MinSimpleDiscountBolt | user wallet ↔ bucket | E2E B7 + listing unit |

The **Playwright E2E** that walks B1→B7 across the four sites is the top-level acceptance test.

---

## TDD workflow

For every phase: **(1) Red** — add the failing test(s) named below; **(2) Green** — implement the minimum to pass; **(3) Refactor** — clean up, keep green. No production code without a failing test first. The golden-vector harness (Phase 1) is the backbone: a ported op is "done" only when its locking script, unlocking script, and txid are **byte-identical** to known-good `ts-bolt` output.

---

## Phase 0 — `ChainBackend` over Arcade (`ttn`)
**Red:** `chain::arcade` test — broadcast a P2PKH tx and round-trip its status+BUMP proof from a (recorded/mock) Arcade response; `ChainBackend::from_settings` resolves the three URLs + `local`.
**Green:**
- `rust-wallet/src/chain/{mod,arcade}.rs`: `ChainBackend{Mainnet,Ttn,Testnet,Local(url)}` → broadcast/proof/header URLs + `has_address_indexer`. `arcade.rs`: `broadcast` (`POST /tx`, octet-stream), `get_tx_status` (`GET /tx/:txid` → BUMP `merklePath`, already consumed by `beef.rs`), headers via `/chaintracks/v1/*`.
- Route existing broadcast (`handlers.rs:~8219`), proof fetch (`cache_helpers.rs`, `monitor/task_check_for_proofs.rs`), height/header (`handlers.rs:~11666`) through `ChainBackend`. Gate WoC UTXO fetch (`utxo_fetcher.rs`, `task_sync_pending.rs`) on `has_address_indexer`.
- **Funding (keep-today/improve):** today = poll WoC per-address into `outputs`, `create_action` selects. Keep WoC for `main`/`testnet`; for Arcade-only `ttn`, **self-track** outputs the wallet creates (insert at build time, confirm via `GET /tx/:txid`) + seed initial funds via `POST /wallet/import-funding` (raw faucet tx + vout, reuses output-insert path).
**Verify:** P2PKH send on `ttn` → `202` + BUMP proof stored; existing wallet tests green on `main`.

## Phase 1 — sx template engine + golden-vector harness (de-risking core)
**Red:** `cargo test bolt_golden` loads fixtures and asserts byte-identical scripts/txid — initially failing. Fixture emitter (`ts-bolt/scripts/emit-golden.mjs`) walks existing signed-tx sims/tests (`ts-bolt/test/**`, `simpleMultiBolt.test.js`, `smb_green.test.js`, `minSimpleBalanceBolt`, `minSimpleDiscountBolt` suites) → `{op, inputs, expected:{lockHex,unlockHex,txid,rawTxHex}}`.
**Green:** `rust-wallet/src/bolt/sx_template.rs` — generic filler consuming a `.sx.json` (`lockOps`/`unlockOps` + `recombinants`) + named-arg map → script bytes (one-time port of the `@elas_co/ts` Tx fill step). `include_str!` embed the demo artifacts. `rust-wallet/src/bolt/lib.rs` — port `boltLib` helpers (`buildOutpoint`, `getAncestorPiece[Fungible]`, `verifyTx/verifyTx2`, `splitCtx`).
**Reuse (do not re-implement):** `transaction/sighash.rs` (ForkID/BIP143 preimage), `transaction/types.rs`, `crypto/signing.rs`, `script/parser.rs`, `beef.rs`.
**Verify:** MSBBolt mint locking script matches a fixture exactly.

## Phase 2 — MSBBolt identity: mint / transfer / send-to-self + SPV export → **B1, B2**
**Red:** golden vectors for MSBBolt mint/transfer; `spv_proof` test (exported BEEF verifies via `smb-payments` verifier against an Arcade/WoC ChainTracker).
**Green:** `rust-wallet/src/bolt/msbbolt.rs` (lock-arg bytes + the unlock args, driving `sx_template`+`sighash`+`signing`); `GET /bolt/proof/:tokenId` exporting a BEEF SPV bundle.
**Verify:** mint identity NFT to a chosen pubkey on `ttn`; self-transfer; export proof that the demo site verifies.

## Phase 3 — SimpleMultiBolt: receive / transfer / melt + currency → **B3, B5**
**Red:** golden vectors for SimpleMultiBolt transfer/melt; decode test (16B LE balance + `currency`); receive test (issuer-minted token internalized into `outputs`).
**Green:** `rust-wallet/src/bolt/simple_multi.rs` (port `SimpleMultiBolt.ts` arg builders, commit→settle two-tx flow); handlers to receive + pay.
**Verify:** receive a Node-issuer-minted £100 IRP token; pay part of it with a coupon applied on `ttn`.

## Phase 4 — MinSimpleDiscountBolt: receive / transfer / redeem / list → **B4, B6, B7**
**Red:** golden vectors for discount mint/transfer/melt; `list_for_sale` unit (build a signed offer/listing artifact priced in IRP).
**Green:** `rust-wallet/src/bolt/discount.rs`; redeem path (consume coupon during a purchase); a minimal **listing** mechanism (signed sell-offer the exchange can accept — escrow/swap-intent; document the trust model for the demo).
**Verify:** receive 10%/5% coupons; redeem one on purchase; list the 5% for £3.33 on bucket.shop.

## Phase 5 — dApp Provider API + approval / auto-register / toasts (C3)
**Red:** Rust handler tests for `/bolt/dapp/*` (domain-scoped); a provider-injection test (page calls `window.hodosBrowser.bolt.*`, request reaches Rust with `X-Requesting-Domain`); auto-approve policy test.
**Green:**
- `rust-wallet/src/handlers/bolt_handlers.rs` + `/bolt` scope in `main.rs`: `requestMint`, `requestTransfer` (issuer→user receive), `getSpvProof`, `listToken`, `getTokens`; per-site **account binding** (`account1` ↔ a BRC-42-derived `usersChosenPubKey`); auto-register (silent/quick-approve in a demo mode).
- C++: add `/bolt` to `isWalletEndpoint()` (`HttpRequestInterceptor.cpp`) + a BOLT approval overlay reuse; inject `window.hodosBrowser.bolt.*` in `simple_render_process_handler.cpp`.
- React: `frontend/src/hooks/useBolt.ts`, flesh out `TokensTab.tsx` (list/mint/send/redeem/list), and **toasts** for mint/receive/redeem events (per Hodos CEF input + overlay rules).
**Verify:** a stub page mints + receives + proves a token via the provider, sees toasts, and binds to `account1`.

## Phase 6 — Demo sites + issuer backends + SPV verify
**Red:** per-site contract test: issuer backend mints the right contract to a given pubkey and returns BEEF; the site verifies an incoming SPV proof (`smb-payments` verifier).
**Green:** `bolt-demo/` with static sites **catpicz.xyz / bwanq.xxx / tackle.new / bucket.shop** (served locally; navigated via the browser) + thin **Node issuer backends** reusing `bolt-wallet`/`ts-bolt` for bwanq (SimpleMultiBolt loan) and tackle (discount coupons), and `smb-payments`-based SPV verification for catpicz/bwanq.
**Verify:** each site, hit directly, performs its beat.

## Phase 7 — Full demo E2E (acceptance)
**Red:** Playwright spec `bolt-demo/test/demo.pw.ts` encoding B1→B7 (extends existing patterns in `bolt-wallet/test/wallet-e2e.pw.ts` / `wallet-sim`).
**Green:** wire timing/UX so the whole journey runs; toasts + account auto-register land at the right beats.
**Verify:** one green E2E run executes the entire narrative on `ttn`.

## Engine completeness (follow-on, after demo green)
Port remaining full-suite contracts to Rust with the same golden-vector discipline: plain **SimpleBolt** (NFT), **MultiBolt** (with swap), **EventListener/EventTrigger**. Each: Red fixtures → Green builder → byte-match.

---

## Critical files
**Create:** `rust-wallet/src/chain/{mod,arcade}.rs`; `rust-wallet/src/bolt/{mod,sx_template,lib,msbbolt,simple_multi,discount}.rs`; `rust-wallet/src/handlers/bolt_handlers.rs`; `rust-wallet/tests/bolt_golden/`; `ts-bolt/scripts/emit-golden.mjs`; `frontend/src/hooks/useBolt.ts`; `bolt-demo/` (4 sites + issuer backends + `test/demo.pw.ts`).
**Modify:** `rust-wallet/src/main.rs` (routes, `ChainBackend` in `AppState`); `handlers.rs` (broadcast/height via ChainBackend); `cache_helpers.rs` + `monitor/task_check_for_proofs.rs` (proof source); `utxo_fetcher.rs`/`task_sync_pending.rs` (gate on indexer); `cef-native/src/core/HttpRequestInterceptor.cpp` (`isWalletEndpoint`); `cef-native/src/handlers/simple_render_process_handler.cpp` (inject `window.hodosBrowser.bolt`); `frontend/src/components/wallet/TokensTab.tsx`.
**Embed/read-only (from priv-chain):** `ts-bolt/src/sxFiles/*.sx.json`, `simplemultibolt/src/contracts/SimpleMultiBolt.sx.json`, `sx/bolt/production/*` (MSBBolt, MinSimpleDiscountBolt, SimpleMultiBolt).

## Reuse map (need → existing)
sighash preimage → `transaction/sighash.rs` · ECDSA → `crypto/signing.rs` · tx/script/varint → `transaction/types.rs` · script parse → `script/parser.rs` · SPV/BEEF/BUMP → `beef.rs`/`beef_helpers.rs` · funding+change+broadcast lifecycle → `create_action`/`sign_action` + Monitor · token keys → `crypto`+`brc42.rs`/`recovery.rs` · issuer minting (sites) → `bolt-wallet`/`ts-bolt` · SPV verify (sites) → `smb-payments/src/verifyPayment.ts`+`smbToken.ts` · E2E harness → `bolt-wallet/test/wallet-e2e.pw.ts`.

## Test pyramid (definition of done)
1. `cargo test bolt_golden` — byte-identical lock/unlock/txid vs ts-bolt fixtures (per-phase gate).
2. Rust handler + SPV-export unit tests (`/bolt/*`, proof bundles verify).
3. Site issuer + `smb-payments` SPV-verify unit tests.
4. Provider-API/auto-approve integration tests.
5. **Playwright B1→B7** on `ttn` — acceptance.
Plus regression: existing BSV send on `main`; Hodos Minimal browser test.

## Biggest risks & mitigations
- **Unlock-arg / ancestor-reconstruction correctness** (pick depths) — historical BOLT pain → golden-vector harness catches every byte diff before broadcast.
- **sx template fill fidelity** (`recombinants`) → validated in Phase 1 on lock-script fixtures first.
- **Arcade ≠ ARC + no UTXO endpoint** → isolated in `arcade.rs`; UTXO gap via self-tracking + faucet import.
- **Two parties minting** (user self-issues identity; sites issue loan/coupons) → clean split: Rust = wallet ops; Node ts-bolt = site issuers.
- **Exchange "listing" trust model** → keep the demo's bucket.shop offer mechanism explicit (signed sell-offer; not a trustless atomic swap unless EventListener is brought in later).
- **DB schema** → reuse `outputs`+`baskets`(`name='bolt_tokens'`, metadata in `customInstructions`); any V25 migration gated on explicit approval (Hodos invariant).
