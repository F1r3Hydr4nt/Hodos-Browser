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
- [x] D-1 ChainBackend enum + from_name/from_opt/from_env resolves local-node/testnet/ttn/main/local-arcade (default local-node; typo never selects mainnet) + arcade base URLs + has_address_indexer + bolt_enabled. DONE Hodos `43bc86d`, 6/6 green. NOTE: Broadcaster/ProofSource/HeaderSource trait defs deferred into D-2/D-3 (kept D-1 to pure config resolution).
- [x] D-2a node.rs: pure build_rpc_body + parse_rpc_response/parse_txid/parse_block_hashes (recorded responses incl. -26 reject) + thin NodeRpc (send_raw_transaction, generate_to_address). DONE Hodos `1624471`, 6/6 green (12/12 chain). Pattern: keep HTTP thin, unit-test the pure request-build/response-parse.
- [x] D-2b arcade.rs: pure build_submit_url + parse_submit_response (202 submitted/already-submitted, 400 Rejected(reason), 503 Backpressure, other Unexpected) + thin ArcadeClient octet-stream broadcast. DONE Hodos `3d37d93`, 7/7 green (19/19 chain).
- [x] D-3 proof: arcade parse_tx_status -> TxStatus{merkle_path BUMP, is_mined, block height/hash}, 404 NotFound; node parse_tx_confirmation -> TxConfirmation{confirmations, blockhash}. DONE Hodos `f52df6f`, 6/6 green (25/25 chain). merkle_path is BUMP hex ready for beef.rs (wiring in D-4).
- [ ] D-4 route existing broadcast/proof/height through ChainBackend; gate WoC UTXO on has_address_indexer [D-2a,D-2b,D-3]
- [ ] D-5 self-track outputs + POST /wallet/import-funding - test [D-4]

### Track B - Contract engine
- [x] Aprime-1 artifactExport.test.js freezes the 7 compiled production artifacts to **sx/bolt/production/artifacts/*.json** (drift-gated by recompilation; LF-pinned). DONE priv-chain `22c8260`. NOTE: emitted in priv-chain (co-located, single-repo) not rust-wallet; B-0 copies/embeds them into rust-wallet. Each artifact has filename/lockArgs/unlockArgs/lockOps/unlockOps/locking+unlockingRecombinants (lockOps present, e.g. MinSimpleBolt lockOps[855]/unlockOps[380]).
- [x] B-0 sx_template.rs filler (Artifact embed + push_data minimal + fill_locking_script mirroring simulator processRecombinants) + MinSimpleBolt lock-script golden match (1245 B, byte-exact vs @bsv). DONE priv-chain `7d8795c` (golden gen) + Hodos `e0119da`, 3/3 green. NOTE: boltLib arg-derivation helpers (buildOutpoint/getAncestorPiece/verifyTx) deferred to B-2 where real mints test-drive them (avoid untested code). Fill model learned: lockingRecombinants = [argIdx ints..., literal hex tail]; string->raw bytes, number->push lockArgs[idx].
- [x] B-1 emitGolden.test.js (jest; sx/src is ESM so plain-node require fails) runs SxSimulator -> per-tx {txid,rawHex,outs[lockHex],ins[unlockHex]} fixtures + determinism gate. DONE priv-chain `c71ac72` + Hodos fixture `9f15e19`. SCOPED to MinSimpleBolt (MSBolt 6-tx lifecycle: tx0 mint out0 lock=1265B); MSBBolt/SimpleMultiBolt/Discount are one-line CASES adds done by B-3/B-4/B-5. simTxs Tx API: .id(), .toHex()/.toBuffer(), .txOuts[i].script.toHex(), .txIns[i].script.toHex().
- [~] B-2 MinSimpleBolt mint/transfer/melt - golden byte-match [B-0,B-1]. **mint-lock DONE** Hodos `7ae58ff` (mint_lock_args derives genesis 6 args; lock matches fixture tx0 byte-for-byte; hash160 boltLib helper added). **unlock ASSEMBLY DONE** Hodos `873cca9`: filler generalized (fill_script + fill_unlocking_script); tx1 bolt-input unlock (2078 B = 37 pushes + 414 B literal) reproduced byte-for-byte from the 37 decoded args. Unlock uses SAME fill model as lock. REMAINING (B-2-unlock-DERIVE, HARD): (a) full mint TX build+txid (P2PKH-funded input + bolt out + change), (b) DERIVE the 37 unlockArgs from spend context + ancestor tx (ancestor reconstruction = historical crux). unlockArgs groups: ancestor* (reconstruct parent tx), current-spend (fundOutpoint, changeOutput, beneficiaryPubKeyHash, sig, pubKey, ctxHeader, ctxCodeLen, ctxCodeUnlock/LockScriptCode, ctxFooter, ctxCodeLockLen). **31/37 DERIVED** Hodos `01fcf3f`: for a GENESIS spend all 26 ancestor* are EMPTY (ancestor reconstruction only for tx2+); 5 contract-static derived (pubKey=owner, ctxCodeLockScriptCode==mint lock, ctxCodeLockLen=varint(1265), ctxCodeUnlockScriptCode=ad68, ctxCodeLen=varint(1267)). REMAINING 6 tx-dependent: **ctxHeader + ctxFooter DONE** Hodos `99020f1` (bolt/ctx.rs BIP143 builder: sha256d + hash_prevouts/sequence/outputs + ctx_header(104)/ctx_footer(52); match tx1 fixture byte-for-byte; sighashType 0x41 ALL|FORKID). 9/9 bolt green. **ALL 37 genesis-spend unlock args ASSEMBLED** Hodos `dcf2bac`: genesis_spend_unlock_args merges static + per-spend (fundOutpoint, changeOutput, beneficiaryPubKeyHash, ctxHeader/ctxFooter via bolt::ctx, sig); full tx1 unlock (2078B) byte-matches fixture (fundOutpoint/changeOutput independently cross-checked). 10/10 bolt green. CTX layout CONFIRMED: preimage = ctxHeader(104) ++ varint(scriptCodeLen=1267) ++ scriptCode(lock 1265 ++ ad68) ++ ctxFooter(52). tx1 wire: version 02000000, locktime 0, 2 inputs (bolt vin0 + funding), 3 outputs.
  **KEY FINDING + tx1/2/3 DONE** Hodos `175ae91`: decoding ALL lifecycle spends shows ONLY tx4 populates the 26 ancestor* args; tx1/2/3/5 are ancestor-less. simple_spend_unlock_args (general; takes the spent UTXO lock script, since owner pubKeyHash changes across transfers while issuer stays fixed) reproduces tx1/tx2/tx3 unlocks byte-for-byte (derived ctxHeader/ctxFooter via BIP143 + fundOutpoint cross-checked; changeOutput/beneficiary/sig = wallet-chosen spend params). 11/11 bolt green. Combined fixtures: minsimplebolt_spend_tx{1,2,3}.json (emit_spend_fixture.cjs TX_IDX=N).
  REMAINING for B-2 full: (1) live SIGN (sig with owner privkey over sha256d(preimage); secp256k1 present; wallet has key). (2) full mint TX build + txid. (3) **ancestor reconstruction - ISOLATED to the tx4-type spend** (decode tx4: all 26 ancestor* populate by reconstructing the parent tx [ancestorVer/Vin1*/Vin2*/Vout1*/Change*/NLockTime]; port getAncestorPiece). (4) melt (tx5, ancestor-less, small unlock 556B - likely a simpler variant). The bulk of MinSimpleBolt spends (mint + 3 transfers) is DERIVED. Decoded args fixture: rust-wallet/tests/fixtures/minsimplebolt_unlock_tx1.json. Genesis arg facts: txoType mint=0x00, commitment=20 zeros, parent/grandparent=36 zeros, issuer=owner pubkey, pubKeyHash=hash160(owner).
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
- 2026-06-16 D-1 GREEN (6/6): ChainBackend module in rust-wallet (`43bc86d`); first Rust code + cargo test proven in-loop (~36s cold build). Next eligible: B-0 (sx_template filler, critical path) or D-2a/D-2b (need only D-1).
- 2026-06-16 D-2a GREEN (`1624471`) + D-2b GREEN (`3d37d93`): node + arcade broadcasters, 19/19 chain. Next eligible: D-3 (proof sources) then D-4/D-5, and B-0 (critical-path engine) in Track B.
- 2026-06-16 D-3 GREEN (`f52df6f`): proof parsers, 25/25 chain. D-4 (wire ChainBackend into existing broadcast/proof/height + gate WoC) and D-5 (self-track + import-funding) TOUCH existing handlers.rs/cache_helpers/monitor - larger integration. B-0 (sx_template engine) is the Track B critical path and is self-contained. Recommend B-0 next.
- 2026-06-16 B-0 GREEN (priv-chain `7d8795c` + Hodos `e0119da`): sx_template filler + MinSimpleBolt lock golden byte-match. Fill model proven. Next eligible: B-1 (emit-golden lifecycle fixtures, priv-chain) then B-2 (MinSimpleBolt mint/transfer/melt + boltLib helpers). B-2 needs full mint arg-derivation (pubKeyHash from key, outpoints, txoType) + the UNLOCK side (ancestor reconstruction) - the hard part.
- 2026-06-16 B-1 GREEN (priv-chain `c71ac72` + Hodos `9f15e19`): lifecycle golden emitter + MinSimpleBolt fixture. Next: B-2. **HARD FRONTIER**: B-2 mint = build full tx (derive pubKeyHash from key, outpoints, txoType=settle 0x20; lock via B-0 filler; P2PKH-funded input+change; match fixture tx0 out0 lockHex + ideally txid). Then transfer commit/settle = the UNLOCK side (37 unlockArgs, ancestor reconstruction) - historically multi-session. Suggest splitting B-2 into B-2-mint-lock (match tx0 out0 lockHex, achievable) then B-2-unlock (ancestor).
