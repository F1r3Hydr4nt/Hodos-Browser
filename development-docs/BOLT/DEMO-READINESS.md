# BOLT × spv-demo-wapps — Demo Readiness & Gap Report

**Date:** 2026-06-17
**Scope of this exercise:** *smoke only, hermetic chain* — run today's pieces, prove what
works, and honestly enumerate the gap to a browser-driven demo. **No feature code was written.**

---

## ⬆️ STATUS UPDATE (2026-06-17, later) — the gap below is now CLOSED

The browser-driven demo described as "missing" below has since been **built and verified
end-to-end** (B1→B7 against the live stack). Highlights:

- **Identity = MinSimpleBolt** (plain NFT) on catpicz/bwanq, both repos; cross-repo proof intact.
- **Wallet `/bolt/*` API** (Layer E): `/bolt/identity/mint`, `/bolt/identity`, `/bolt/pay` (SMB IRP
  payment, B5), `/bolt/sign` (offer signing, B7). No C++ rebuild — CEF passes non-wallet-endpoint
  `:31301` calls through; the wallet CORS allowlists the demo origins (`localhost:3001-3004`).
- **Shared hermetic chain service** (`spv-demo-wapps/apps/chain`, :3010, `CHAIN_BACKEND=shared`):
  one broadcaster+tracker so a BEEF the wallet mints verifies at every site across docker containers.
- **Interactive frontends** on all four sites (replacing the static doc pages), driving the wallet
  automatically in the background. **catpicz**: cat gallery with the premium download **greyed out
  until you register** via the MinSimpleBolt identity (the headline). **tackle**: members-only
  identity gate before claiming coupons.
- **Verified:** Rust 35 bolt + 30 chain lib tests; demo 45 vitest (incl. full B1→B7 + cross-repo);
  and a live curl walk of B1→B7 against the running stack — all green.
- **Outstanding:** drop the two cat images into `apps/catpicz/public/{bomb-cat,sneaky-cats}.jpg`
  (the page shows a placeholder until then). 4 pre-existing Rust lib failures in
  `certificate::selective_disclosure` (DB FK) + `utxo_fetcher` (network) are unrelated to this work.

The sections below are the original smoke-phase gap analysis, kept as the historical baseline.

---

## TL;DR

The BOLT **engine** and the **demo backends** are real and green, and they already interoperate
(a real Rust-wallet-minted token is accepted by the demo sites in an automated test). The
**browser, wallet daemon, and demo stack all run.** What does **not** exist yet is the wiring that
lets the *browser* drive BOLT against the sites: the wallet has **no `/bolt/*` HTTP surface**, the
page provider has **no `window.hodosBrowser.bolt`**, there is **no BOLT UI**, and the demo sites are
**REST + static pages with no client JS**. So today you can *watch the parts run*, but you cannot yet
*click through B1→B7 in the browser*. The ordered path to that is below.

**Verdict:** Engine ✅ · Demo backends ✅ · Interop (test-level) ✅ · Browser/wallet/stack boot ✅ · **Browser-driven token flow ❌ (Layers E + F unbuilt).**

---

## What was run today (and the results)

| Check | Command | Result |
|---|---|---|
| Rust BOLT engine | `cargo test --lib bolt` | **32 passed / 0 failed** |
| Rust chain resolver | `cargo test --lib chain` | **30 passed / 0 failed** (29 `chain::*` + 1 `arc_status`) |
| Demo apps + interop | `npm test` (vitest) in `spv-demo-wapps` | **15 files / 40 tests passed** |
| → full B1→B7 acceptance | `test/demo.e2e-spec.ts` | ✅ one shared hermetic chain |
| → cross-repo interop | `test/hodos-wallet.e2e-spec.ts` | ✅ 2/2 — real wallet mint accepted by catpicz + bwanq |
| Demo stack | `docker compose up -d --build` | ✅ postgres + catpicz/bwanq/tackle/bucket **Up (healthy)** |
| Site health | `GET :3001-3004/health` | ✅ `200 {"status":"ok"}` each |
| Wallet daemon | `GET :31301/wallet/balance` | ✅ `200 {"balance":0,"bsvPrice":0.0}` |
| Wallet token list (TokensTab source) | `GET :31301/wallet/tokens` | ✅ `200 {"tokens":[],"count":0}` |
| Frontend dev server | Vite on `:5137` | ✅ `200` (v6.3.5) |
| CEF browser shell | `HodosBrowserShell.exe` (HODOS_DEV=1) | ✅ running (13-process CEF tree) |

---

## Proven today (with evidence)

1. **The ported engine is byte-correct.** All four production contracts (MinSimpleBolt, MSBBolt,
   SimpleMultiBolt, MinSimpleDiscountBolt) fill lock/unlock scripts that byte-match ts-bolt across
   every spend shape (mint/transfer/settle/split/merge/melt/ancestor) — `rust-wallet/src/bolt/*`, 32 tests.
2. **The chain resolver is correct.** `ChainBackend` routing (local-node/testnet/ttn/main/local-arcade),
   Arcade + node clients, proof parsing — `rust-wallet/src/chain/*`, 30 tests. (Pure resolver; not yet
   wired into the live money path — see D-4b below.)
3. **The demo narrative passes end-to-end.** `spv-demo-wapps/test/demo.e2e-spec.ts` walks B1→B7
   (identity → bank loan → coupon → pay+redeem → further coupon → list-for-sale) on one hermetic chain.
4. **Engine ↔ demo interoperate for real.** `spv-demo-wapps/test/hodos-wallet.e2e-spec.ts` feeds the
   *actual Rust-wallet-minted* identity tx (`rust-wallet/tests/fixtures/b1_identity_mint.json`) through
   the demo's own `HermeticBroadcaster.getProof` and asserts catpicz **and** bwanq `/identity/verify`
   accept it and decode the wallet's pubKeyHash. This is the key cross-repo proof.
5. **All three runtime layers boot:** wallet daemon (31301), Vite frontend (5137), and the prebuilt
   CEF browser — and the demo stack is reachable in-browser (`GET localhost:3001/` → 200, page titled
   "catpicz.xyz — sign in with your identity token").

---

## The gap: why you can't click through B1→B7 in the browser yet

| # | Missing piece | Evidence it's absent | Plan ref |
|---|---|---|---|
| 1 | **Wallet `/bolt/*` HTTP surface** — `/bolt/{mint,transfer,melt,receive}` + `GET /bolt/proof/:id`. BOLT is library-only; nothing exposes it over HTTP. | No `/bolt` routes in `rust-wallet/src/main.rs` (only `/wallet/tokens`); no `handlers/bolt_handlers.rs`. | **E-1, E-2** |
| 2 | **A B1→B7 driver** that turns a wallet raw tx into a hermetic BEEF the sites accept. Today only the in-process test does this (it holds the broadcaster handle). | `getProof` is internal to the demo; the wallet has no way to obtain a hermetic BEEF out-of-process. | (glue) |
| 3 | **Provider injection** — `window.hodosBrowser.bolt.*` — and C++ `isWalletEndpoint()` matching `/bolt/*`. | Frontend has **zero** `/bolt` / `hodosBrowser.bolt` references; provider exposes only wallet/brc100/navigation/overlay. | **F-1** |
| 4 | **BOLT UI** — `frontend/src/hooks/useBolt.ts` + a real `TokensTab.tsx` + toasts. | `TokensTab.tsx` is a placeholder hitting generic `/wallet/tokens`; no `useBolt.ts`. | **F-3** |
| 5 | **Interactive demo-site frontends** that call the provider. | Sites are REST + static doc pages; `GET /` HTML has no `<script>`, no `fetch`, no provider calls. | **F-2 / demo** |
| 6 | *(Live chains only — not needed hermetic)* **D-4b/D-5 money-path** wiring + funding. | `endpoints.rs` is a pure resolver; broadcast/proof/height still use the legacy WoC/ARC paths. | **D-4b, D-5** |

Status against the backlog: **A ✅ · A′ ✅ · B ✅ · C 🟡(deferred) · D 🟡(D-1/2/3/4a done, D-4b/5 pending) · E ❌ · F ❌.** See `PROGRESS.md` and `BOLT-integration-plan.md`.

---

## Operational notes / snags found (worth fixing before a click-through demo)

- **`frontend/node_modules/` was an empty directory** (0 packages) → `vite` missing → `npm run dev`
  failed with *"'vite' is not recognized"*. Fixed for this run with `npm install` (176 pkgs).
- **`run-dev.bat` (untracked) is unreliable for this:** (a) its `if not exist node_modules` guard is
  fooled by the *empty* dir, so it skips the install and hits the same vite failure; (b) it launches the
  wallet with bare `cargo run --release` **without `HODOS_DEV=1`**, which the dev-safeguard rejects when
  running from a build dir. The CLAUDE.md-sanctioned scripts are `dev-wallet.ps1` + `win_build_run.sh`.
- **Release binaries weren't built**, so the wallet ran in **debug** for the smoke (functionally fine for HTTP).
- **Launching the CEF GUI from a background/non-interactive shell fails** (exits right after
  "Shell starting…", code 127). It launches correctly via PowerShell **`Start-Process`** in the
  interactive session (13-process CEF tree). The browser's C++ history DB used the *production* data dir
  (`%APPDATA%/HodosBrowser/Default`) even with `HODOS_DEV=1` — browser-data isolation may not honor the
  flag the way the wallet does; worth confirming separately (not blocking).

---

## How to reproduce today's smoke

```sh
# 1. Demo stack (hermetic, offline)
cd spv-demo-wapps && docker compose up -d --build
for p in 3001 3002 3003 3004; do curl -s localhost:$p/health; echo; done

# 2. The evidence suites
cd spv-demo-wapps && npm test                       # 15 files / 40 tests (incl. demo + hodos-wallet)
# Rust engine (Hodos-Browser/rust-wallet):
source ~/.vsenv.sh
"$HOME/.cargo/bin/cargo.exe" test --manifest-path rust-wallet/Cargo.toml --lib bolt    # 32
"$HOME/.cargo/bin/cargo.exe" test --manifest-path rust-wallet/Cargo.toml --lib chain   # 30

# 3. Browser (three pieces; do NOT rely on run-dev.bat — see snags)
#   wallet  : cd rust-wallet && HODOS_DEV=1 cargo run --bin hodos-wallet      # :31301
#   frontend: cd frontend && npm install && npm run dev                       # :5137
#   browser : Start-Process cef-native/build/bin/Release/HodosBrowserShell.exe  (env HODOS_DEV=1)
# Then navigate the browser to http://localhost:3001 .. :3004 (static pages render; no token flow yet).
```

---

## Recommended next step

If you want the in-browser click-through, the smallest first increment is **Layer E** (the
`/bolt/*` daemon handlers) + a hermetic **B1→B7 driver** (extend the in-process
`hodos-wallet.e2e-spec.ts` pattern, or add a tiny proof-broker endpoint). That makes the whole
journey runnable from outside the test harness without yet touching C++/React or the money path.
Layer F (provider + UI + demo frontends) turns that into the actual browser experience.
